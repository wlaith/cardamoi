use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::flaresolverr::FlareSolverrClient;

#[derive(Debug, Clone, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub year: Option<i32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    #[serde(default)]
    pub match_title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub source: String,
    pub title: String,
    pub year: Option<i32>,
    pub size_bytes: Option<u64>,
    pub seeders: Option<u32>,
    pub torrent_url: String,
    pub poster: Option<String>,
    pub details_url: Option<String>,
    pub single_video: bool,
}

pub trait Source: Send + Sync {
    fn name(&self) -> &str;
    fn search<'a>(&'a self, query: &'a SearchQuery) -> BoxFuture<'a, anyhow::Result<Vec<SearchResult>>>;
}

pub struct InternetArchive {
    pub http: Arc<FlareSolverrClient>,
}

#[derive(Deserialize)]
struct IaResponse {
    response: IaDocs,
}

#[derive(Deserialize)]
struct IaDocs {
    docs: Vec<IaDoc>,
}

#[derive(Deserialize)]
struct IaDoc {
    identifier: String,
    title: Option<IaText>,
    year: Option<IaNumber>,
    item_size: Option<u64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum IaText {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum IaNumber {
    Int(i32),
    Text(String),
}

impl Source for InternetArchive {
    fn name(&self) -> &str {
        "Internet Archive"
    }

    fn search<'a>(&'a self, query: &'a SearchQuery) -> BoxFuture<'a, anyhow::Result<Vec<SearchResult>>> {
        Box::pin(async move {
            let cleaned: String = query
                .text
                .chars()
                .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
                .collect();
            let mut q = format!("title:({}) AND mediatype:movies", cleaned.trim());
            if let Some(year) = query.year {
                q.push_str(&format!(" AND year:{year}"));
            }

            let resp: IaResponse = self
                .http
                .get("https://archive.org/advancedsearch.php")
                .query(&[
                    ("q", q.as_str()),
                    ("fl[]", "identifier"),
                    ("fl[]", "title"),
                    ("fl[]", "year"),
                    ("fl[]", "item_size"),
                    ("sort[]", "downloads desc"),
                    ("rows", "40"),
                    ("output", "json"),
                ])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;

            Ok(resp
                .response
                .docs
                .into_iter()
                .map(|doc| {
                    let id = doc.identifier;
                    let title = match doc.title {
                        Some(IaText::One(t)) => t,
                        Some(IaText::Many(t)) => t.into_iter().next().unwrap_or_else(|| id.clone()),
                        None => id.clone(),
                    };
                    let year = match doc.year {
                        Some(IaNumber::Int(y)) => Some(y),
                        Some(IaNumber::Text(y)) => y.get(..4).and_then(|y| y.parse().ok()),
                        None => None,
                    };
                    SearchResult {
                        source: self.name().to_owned(),
                        title,
                        year,
                        size_bytes: doc.item_size,
                        seeders: None,
                        torrent_url: format!("https://archive.org/download/{id}/{id}_archive.torrent"),
                        poster: Some(format!("https://archive.org/services/img/{id}")),
                        details_url: Some(format!("https://archive.org/details/{id}")),
                        single_video: true,
                    }
                })
                .collect())
        })
    }
}

pub struct Torznab {
    pub http: Arc<FlareSolverrClient>,
    pub name: String,
    pub url: String,
    pub api_key: String,
}

impl Torznab {
    fn endpoint(&self) -> String {
        let url = self.url.trim().trim_end_matches('/');
        if url.ends_with("/api") { url.to_owned() } else { format!("{url}/api") }
    }

    async fn get(&self, params: &[(&str, String)]) -> anyhow::Result<String> {
        let mut query: Vec<(&str, String)> = vec![("apikey", self.api_key.clone())];
        query.extend_from_slice(params);
        let resp = self.http.get(&self.endpoint()).query(&query).send().await?;
        let body = resp.error_for_status()?.text().await?;
        let doc = roxmltree::Document::parse(&body).map_err(|_| anyhow::anyhow!("the response isn't a Torznab feed"))?;
        let root = doc.root_element();
        if root.has_tag_name("error") {
            anyhow::bail!("{}", root.attribute("description").unwrap_or("the source returned an error"));
        }
        Ok(body)
    }

    pub async fn test(&self) -> anyhow::Result<()> {
        let body = self.get(&[("t", "caps".into())]).await?;
        let doc = roxmltree::Document::parse(&body)?;
        anyhow::ensure!(doc.root_element().has_tag_name("caps"), "the response isn't a Torznab feed");
        Ok(())
    }
}

fn parse_torznab(source: &str, body: &str) -> anyhow::Result<Vec<SearchResult>> {
    let doc = roxmltree::Document::parse(body)?;
    let results = doc
        .descendants()
        .filter(|n| n.has_tag_name("item"))
        .filter_map(|item| {
            let child = |name: &str| {
                item.children()
                    .find(|c| c.tag_name().name() == name && c.tag_name().namespace().is_none())
                    .and_then(|c| c.text())
            };
            let attr = |name: &str| {
                item.children()
                    .find(|c| c.tag_name().name() == "attr" && c.attribute("name") == Some(name))
                    .and_then(|c| c.attribute("value"))
            };
            let enclosure = item.children().find(|c| c.has_tag_name("enclosure"));

            let torrent_url = attr("magneturl")
                .or_else(|| enclosure.and_then(|e| e.attribute("url")))
                .or_else(|| child("link"))?
                .to_owned();
            Some(SearchResult {
                source: source.to_owned(),
                title: child("title")?.to_owned(),
                year: attr("year").and_then(|y| y.parse().ok()),
                size_bytes: child("size")
                    .or_else(|| attr("size"))
                    .or_else(|| enclosure.and_then(|e| e.attribute("length")))
                    .and_then(|s| s.parse().ok()),
                seeders: attr("seeders").and_then(|s| s.parse().ok()),
                torrent_url,
                poster: attr("coverurl").map(str::to_owned),
                details_url: child("comments").map(str::to_owned),
                single_video: false,
            })
        })
        .collect();
    Ok(results)
}

impl Source for Torznab {
    fn name(&self) -> &str {
        &self.name
    }

    fn search<'a>(&'a self, query: &'a SearchQuery) -> BoxFuture<'a, anyhow::Result<Vec<SearchResult>>> {
        Box::pin(async move {
            let mut params = vec![("q", query.text.trim().to_owned()), ("extended", "1".into())];
            if let Some(season) = query.season {
                params.push(("t", "tvsearch".into()));
                params.push(("season", season.to_string()));
                if let Some(ep) = query.episode {
                    params.push(("ep", ep.to_string()));
                }
            } else {
                params.push(("t", "search".into()));
                if let Some(year) = query.year {
                    params[0].1.push_str(&format!(" {year}"));
                }
            }
            let body = self.get(&params).await?;
            parse_torznab(&self.name, &body)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flaresolverr::FlareSolverrConfig;

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:torznab="http://torznab.com/schemas/2015/feed">
  <channel>
    <item>
      <title>Some Film 1968 1080p</title>
      <comments>https://indexer.example/details/1</comments>
      <size>2147483648</size>
      <link>https://indexer.example/dl/1.torrent</link>
      <enclosure url="https://indexer.example/dl/1.torrent" length="2147483648" type="application/x-bittorrent" />
      <torznab:attr name="seeders" value="42" />
      <torznab:attr name="magneturl" value="magnet:?xt=urn:btih:abc" />
    </item>
    <item>
      <title>No Magnet</title>
      <enclosure url="https://indexer.example/dl/2.torrent" length="10" type="application/x-bittorrent" />
    </item>
    <item><title>No link at all</title></item>
  </channel>
</rss>"#;

    #[test]
    fn parses_items() {
        let results = parse_torznab("Test", FEED).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].torrent_url, "magnet:?xt=urn:btih:abc");
        assert_eq!(results[0].seeders, Some(42));
        assert_eq!(results[0].size_bytes, Some(2147483648));
        assert_eq!(results[0].details_url.as_deref(), Some("https://indexer.example/details/1"));
        assert_eq!(results[1].torrent_url, "https://indexer.example/dl/2.torrent");
        assert_eq!(results[1].size_bytes, Some(10));
        assert_eq!(results[1].seeders, None);
    }

    async fn fake_torznab() -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = vec![0; 4096];
                let n = sock.read(&mut buf).await.unwrap();
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let body = if !req.contains("apikey=good") {
                    r#"<error code="100" description="Invalid API Key"/>"#.to_owned()
                } else if req.contains("t=caps") {
                    "<caps><searching/></caps>".to_owned()
                } else {
                    FEED.to_owned()
                };
                let resp = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
                sock.write_all(resp.as_bytes()).await.unwrap();
            }
        });
        format!("http://{addr}/1/")
    }

    #[tokio::test]
    async fn talks_to_a_torznab_server() {
        let url = fake_torznab().await;
        let source = |key: &str| Torznab { http: Arc::new(FlareSolverrClient::new(FlareSolverrConfig::default()).unwrap()), name: "Fake".into(), url: url.clone(), api_key: key.into() };

        source("good").test().await.unwrap();
        let bad = source("bad").test().await.unwrap_err();
        assert_eq!(bad.to_string(), "Invalid API Key");

        let query = SearchQuery { text: "some film".into(), year: None, season: None, episode: None, match_title: None };
        let results = source("good").search(&query).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, "Fake");
    }

    #[test]
    fn endpoint_appends_api() {
        let t = |url: &str| Torznab { http: Arc::new(FlareSolverrClient::new(FlareSolverrConfig::default()).unwrap()), name: "x".into(), url: url.into(), api_key: "k".into() };
        assert_eq!(t("http://localhost:9696/1/").endpoint(), "http://localhost:9696/1/api");
        assert_eq!(t("http://localhost:9117/api/v2.0/indexers/all/results/torznab/api").endpoint(), "http://localhost:9117/api/v2.0/indexers/all/results/torznab/api");
    }
}
