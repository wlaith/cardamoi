use serde::{Deserialize, Serialize};

const API: &str = "https://api.themoviedb.org/3";
const IMAGES: &str = "https://image.tmdb.org/t/p";

#[derive(Debug, Clone, PartialEq)]
pub enum Auth {
    Bearer(String),
    ApiKey(String),
}

impl Auth {
    pub fn kind(&self) -> &'static str {
        match self {
            Auth::Bearer(_) => "token",
            Auth::ApiKey(_) => "key",
        }
    }
}

pub fn parse_key(text: &str) -> Option<Auth> {
    let key = text.trim().trim_matches(|c| c == '"' || c == '\'' || c == '`').trim();
    let jwt = key.starts_with("eyJ")
        && key.split('.').count() == 3
        && key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if jwt {
        Some(Auth::Bearer(key.to_owned()))
    } else if key.len() == 32 && key.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(Auth::ApiKey(key.to_ascii_lowercase()))
    } else {
        None
    }
}

#[derive(Debug)]
pub struct Rejected;

impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("tmdb:rejected")
    }
}

impl std::error::Error for Rejected {}

fn image(size: &str, path: Option<String>) -> Option<String> {
    path.map(|p| format!("{IMAGES}/{size}{p}"))
}

fn year(date: Option<&str>) -> Option<i32> {
    date?.get(..4)?.parse().ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Movie,
    Tv,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Title {
    pub id: u64,
    pub kind: Kind,
    pub title: String,
    #[serde(skip)]
    pub original_title: String,
    pub year: Option<i32>,
    pub poster: Option<String>,
    pub backdrop: Option<String>,
    pub overview: String,
    pub rating: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Season {
    pub number: u32,
    pub name: String,
    pub episode_count: u32,
    pub poster: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    #[serde(flatten)]
    pub title: Title,
    pub runtime: Option<u32>,
    pub genres: Vec<String>,
    pub seasons: Vec<Season>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub number: u32,
    pub name: String,
    pub overview: String,
    pub air_date: Option<String>,
    pub still: Option<String>,
}

#[derive(Deserialize)]
struct RawTitle {
    id: u64,
    media_type: Option<String>,
    title: Option<String>,
    name: Option<String>,
    original_title: Option<String>,
    original_name: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    #[serde(default)]
    overview: String,
    vote_average: Option<f32>,
    vote_count: Option<u32>,
    runtime: Option<u32>,
    episode_run_time: Option<Vec<u32>>,
    #[serde(default)]
    genres: Vec<RawGenre>,
    #[serde(default)]
    seasons: Vec<RawSeason>,
}

#[derive(Deserialize)]
struct RawGenre {
    name: String,
}

#[derive(Deserialize)]
struct RawSeason {
    season_number: u32,
    name: String,
    episode_count: Option<u32>,
    poster_path: Option<String>,
}

#[derive(Deserialize)]
struct RawEpisode {
    episode_number: u32,
    name: String,
    #[serde(default)]
    overview: String,
    air_date: Option<String>,
    still_path: Option<String>,
}

#[derive(Deserialize)]
struct Page<T> {
    results: Vec<T>,
}

#[derive(Deserialize)]
struct RawSeasonDetails {
    episodes: Vec<RawEpisode>,
}

impl RawTitle {
    fn into_title(self, kind: Kind) -> Title {
        Title {
            id: self.id,
            kind,
            title: self.title.or(self.name).unwrap_or_default(),
            original_title: self.original_title.or(self.original_name).unwrap_or_default(),
            year: year(self.release_date.as_deref().or(self.first_air_date.as_deref())),
            poster: image("w342", self.poster_path),
            backdrop: image("w1280", self.backdrop_path),
            overview: self.overview,
            rating: self.vote_average.filter(|_| self.vote_count.unwrap_or(0) >= 10),
        }
    }
}

pub struct Tmdb {
    pub http: reqwest::Client,
    pub auth: Auth,
}

impl Tmdb {
    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> anyhow::Result<T> {
        let request = self.http.get(&format!("{API}{path}")).query(query);
        let request = match &self.auth {
            Auth::Bearer(token) => request.bearer_auth(token),
            Auth::ApiKey(key) => request.query(&[("api_key", key)]),
        };
        let resp = request.send().await?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Rejected.into());
        }
        Ok(resp.error_for_status()?.json().await?)
    }

    pub async fn validate(&self) -> anyhow::Result<()> {
        let _: serde_json::Value = self.get("/authentication", &[]).await?;
        Ok(())
    }

    pub async fn search(&self, text: &str) -> anyhow::Result<Vec<Title>> {
        let page: Page<RawTitle> =
            self.get("/search/multi", &[("query", text), ("include_adult", "false")]).await?;
        Ok(page
            .results
            .into_iter()
            .filter_map(|raw| {
                let kind = match raw.media_type.as_deref() {
                    Some("movie") => Kind::Movie,
                    Some("tv") => Kind::Tv,
                    _ => return None,
                };
                Some(raw.into_title(kind))
            })
            .collect())
    }

    pub async fn best_match(&self, title: &str, year: Option<i32>, tv: bool) -> anyhow::Result<Option<Title>> {
        let results = self.search(title).await?;
        let wanted = if tv { Kind::Tv } else { Kind::Movie };
        let norm = crate::release::normalize(title);
        Ok(results
            .into_iter()
            .filter(|t| {
                t.kind == wanted
                    && (crate::release::normalize(&t.title) == norm || crate::release::normalize(&t.original_title) == norm)
            })
            .find(|t| year.is_none() || t.year.zip(year).is_some_and(|(a, b)| (a - b).abs() <= 1)))
    }

    pub async fn details(&self, kind: Kind, id: u64) -> anyhow::Result<Details> {
        let path = match kind {
            Kind::Movie => format!("/movie/{id}"),
            Kind::Tv => format!("/tv/{id}"),
        };
        let mut raw: RawTitle = self.get(&path, &[]).await?;
        let runtime = raw.runtime.or_else(|| raw.episode_run_time.as_ref()?.first().copied());
        let genres = raw.genres.drain(..).map(|g| g.name).collect();
        let seasons = raw
            .seasons
            .drain(..)
            .filter(|s| s.season_number > 0)
            .map(|s| Season {
                number: s.season_number,
                name: s.name,
                episode_count: s.episode_count.unwrap_or(0),
                poster: image("w342", s.poster_path),
            })
            .collect();
        Ok(Details { title: raw.into_title(kind), runtime, genres, seasons })
    }

    pub async fn episodes(&self, show_id: u64, season: u32) -> anyhow::Result<Vec<Episode>> {
        let raw: RawSeasonDetails = self.get(&format!("/tv/{show_id}/season/{season}"), &[]).await?;
        Ok(raw
            .episodes
            .into_iter()
            .map(|e| Episode {
                number: e.episode_number,
                name: e.name,
                overview: e.overview,
                air_date: e.air_date,
                still: image("w300", e.still_path),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_key_formats() {
        let token = "eyJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhYmMifQ.c2lnbmF0dXJl_-";
        assert_eq!(parse_key(token), Some(Auth::Bearer(token.into())));
        assert_eq!(parse_key(&format!("  \"{token}\"\n")), Some(Auth::Bearer(token.into())));
        let key = "0123456789ABCDEF0123456789abcdef";
        assert_eq!(parse_key(key), Some(Auth::ApiKey(key.to_ascii_lowercase())));
        assert_eq!(parse_key(""), None);
        assert_eq!(parse_key("not a key"), None);
        assert_eq!(parse_key("eyJ only one part"), None);
        assert_eq!(parse_key("0123456789abcdef"), None);
    }

    #[tokio::test]
    #[ignore]
    async fn live_lookup() {
        let key = std::env::var("TMDB_KEY").expect("set TMDB_KEY to run this");
        let tmdb = Tmdb { http: reqwest::Client::new(), auth: parse_key(&key).unwrap() };
        let results = tmdb.search("The Matrix").await.unwrap();
        let matrix = results.iter().find(|t| t.kind == Kind::Movie && t.year == Some(1999)).unwrap();
        assert!(matrix.poster.is_some());

        let show = tmdb.best_match("Severance", Some(2022), true).await.unwrap().unwrap();
        let details = tmdb.details(Kind::Tv, show.id).await.unwrap();
        assert!(details.seasons.iter().any(|s| s.number == 1));
        let episodes = tmdb.episodes(show.id, 1).await.unwrap();
        assert_eq!(episodes[0].number, 1);

        let parsed = crate::release::parse("La bola negra [The Black Ball] 10bit 4KDS 1080p SVT-AV1-Tritium.mkv");
        let film = tmdb.best_match(&parsed.title, parsed.year, false).await.unwrap().unwrap();
        assert_eq!((film.id, film.original_title.as_str()), (1422041, "La bola negra"));
    }
}
