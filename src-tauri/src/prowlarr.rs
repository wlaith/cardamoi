use std::{
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

use crate::sources::{SearchQuery, SearchResult, Source};

const PORT: u16 = 9797;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", content = "message", rename_all = "lowercase")]
pub enum Status {
    Starting,
    Ready,
    Failed(String),
}

pub struct Prowlarr {
    pub base_url: String,
    api_key: String,
    child: Mutex<Option<Child>>,
    status: Mutex<Status>,
}

/// Localhost-only, so the login screen can be off.
fn initial_config(api_key: &str) -> String {
    format!(
        "<Config>
  <BindAddress>127.0.0.1</BindAddress>
  <Port>{PORT}</Port>
  <ApiKey>{api_key}</ApiKey>
  <AuthenticationMethod>External</AuthenticationMethod>
  <AuthenticationRequired>DisabledForLocalAddresses</AuthenticationRequired>
  <LaunchBrowser>False</LaunchBrowser>
  <AnalyticsEnabled>False</AnalyticsEnabled>
</Config>
"
    )
}

fn ensure_config(data_dir: &Path) -> anyhow::Result<String> {
    let path = data_dir.join("config.xml");
    if let Ok(xml) = std::fs::read_to_string(&path) {
        let doc = roxmltree::Document::parse(&xml)?;
        if let Some(key) = doc.descendants().find(|n| n.has_tag_name("ApiKey")).and_then(|n| n.text()) {
            return Ok(key.trim().to_owned());
        }
    }
    let key = uuid::Uuid::new_v4().simple().to_string();
    std::fs::write(&path, initial_config(&key))?;
    Ok(key)
}

fn kill_leftovers(binary: &Path) {
    let pattern = binary.to_string_lossy();
    let alive = || {
        Command::new("pgrep")
            .args(["-f", &pattern])
            .stdout(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    if !alive() {
        return;
    }
    let _ = Command::new("pkill").args(["-f", &pattern]).status();
    for _ in 0..30 {
        if !alive() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = Command::new("pkill").args(["-9", "-f", &pattern]).status();
}

impl Prowlarr {
    pub fn start(resource_dir: &Path, data_dir: &Path, http: reqwest::Client) -> Arc<Self> {
        let base_url = format!("http://127.0.0.1:{PORT}");
        let data_dir = data_dir.join("prowlarr");
        let binary = resource_dir.join("prowlarr").join("Prowlarr");

        let launched = (|| -> anyhow::Result<(String, Child)> {
            anyhow::ensure!(binary.exists(), "Prowlarr isn't bundled in this build");
            std::fs::create_dir_all(&data_dir)?;
            let api_key = ensure_config(&data_dir)?;
            kill_leftovers(&binary);
            let child = Command::new(&binary)
                .arg("-nobrowser")
                .arg(format!("-data={}", data_dir.display()))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            Ok((api_key, child))
        })();

        let (api_key, child, status) = match launched {
            Ok((key, child)) => (key, Some(child), Status::Starting),
            Err(e) => (String::new(), None, Status::Failed(format!("{e:#}"))),
        };
        let prowlarr = Arc::new(Self {
            base_url,
            api_key,
            child: Mutex::new(child),
            status: Mutex::new(status),
        });

        if matches!(*prowlarr.status.lock().unwrap(), Status::Starting) {
            let p = prowlarr.clone();
            let http_client = http.clone();
            tauri::async_runtime::spawn(async move {
                for _ in 0..120 {
                    let ping = http_client
                        .get(format!("{}/api/v1/system/status", p.base_url))
                        .header("X-Api-Key", &p.api_key)
                        .timeout(Duration::from_secs(2))
                        .send()
                        .await;
                    match ping.map(|r| r.status()) {
                        Ok(s) if s.is_success() => {
                            *p.status.lock().unwrap() = Status::Ready;
                            return;
                        }
                        Ok(reqwest::StatusCode::UNAUTHORIZED) => {
                            *p.status.lock().unwrap() = Status::Failed(format!("another Prowlarr is already using port {PORT}"));
                            return;
                        }
                        _ => {}
                    }
                    if let Some(Ok(Some(code))) = p.child.lock().unwrap().as_mut().map(|c| c.try_wait()) {
                        *p.status.lock().unwrap() = Status::Failed(format!("Prowlarr exited during startup ({code})"));
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                *p.status.lock().unwrap() = Status::Failed("Prowlarr didn't start within a minute".into());
            });
        }
        prowlarr
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    pub fn stop(&self) {
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Prowlarr {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct ProwlarrSource {
    pub prowlarr: Arc<Prowlarr>,
    pub http: reqwest::Client,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Release {
    title: String,
    size: Option<u64>,
    seeders: Option<u32>,
    indexer: Option<String>,
    magnet_url: Option<String>,
    download_url: Option<String>,
    info_url: Option<String>,
    poster_url: Option<String>,
}

impl ProwlarrSource {
    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> anyhow::Result<T> {
        let resp = self
            .http
            .get(&format!("{}{path}", self.prowlarr.base_url))
            .header("X-Api-Key", &self.prowlarr.api_key)
            .query(query)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp)
    }
}

impl Source for ProwlarrSource {
    fn name(&self) -> &str {
        "Prowlarr"
    }

    fn search<'a>(&'a self, query: &'a SearchQuery) -> BoxFuture<'a, anyhow::Result<Vec<SearchResult>>> {
        Box::pin(async move {
            match self.prowlarr.status() {
                Status::Ready => {}
                Status::Starting => anyhow::bail!("still starting, try again in a few seconds"),
                Status::Failed(e) => anyhow::bail!("{e}"),
            }

            let indexers: Vec<serde_json::Value> = self.get("/api/v1/indexer", &[]).await?;
            if indexers.is_empty() {
                anyhow::bail!("no indexers yet. Add some with Manage indexers on the Sources page.");
            }

            let mut text = query.text.trim().to_owned();
            if let Some(season) = query.season {
                text.push_str(&format!(" S{season:02}"));
                if let Some(ep) = query.episode {
                    text.push_str(&format!("E{ep:02}"));
                }
            } else if let Some(year) = query.year {
                text.push_str(&format!(" {year}"));
            }

            let releases: Vec<Release> =
                self.get("/api/v1/search", &[("query", &text), ("type", "search"), ("limit", "100")]).await?;
            Ok(releases
                .into_iter()
                .filter_map(|r| {
                    Some(SearchResult {
                        source: r.indexer.unwrap_or_else(|| "Prowlarr".into()),
                        title: r.title,
                        year: None,
                        size_bytes: r.size,
                        seeders: r.seeders,
                        torrent_url: r.magnet_url.or(r.download_url)?,
                        poster: r.poster_url,
                        details_url: r.info_url,
                        single_video: false,
                    })
                })
                .collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_key_is_created_once_and_reused() {
        let dir = std::env::temp_dir().join(format!("cardamoi-test-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();

        let key = ensure_config(&dir).unwrap();
        assert_eq!(key.len(), 32);
        let xml = std::fs::read_to_string(dir.join("config.xml")).unwrap();
        assert!(xml.contains("<BindAddress>127.0.0.1</BindAddress>"));

        // The key must survive Prowlarr rewriting the file.
        std::fs::write(dir.join("config.xml"), xml.replace("</Config>", "  <LogLevel>debug</LogLevel>\n</Config>")).unwrap();
        assert_eq!(ensure_config(&dir).unwrap(), key);

        std::fs::remove_dir_all(dir).unwrap();
    }
}
