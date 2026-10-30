use std::sync::{Arc, RwLock};

use anyhow::Context;
use reqwest::{Client, RequestBuilder, Response};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlareSolverrConfig {
    pub enabled: bool,
    pub url: String,
}

impl Default for FlareSolverrConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "http://localhost:8191".to_owned(),
        }
    }
}

#[derive(Serialize)]
struct FlareSolverrRequest {
    cmd: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    post_data: Option<String>,
}

#[derive(Deserialize)]
struct FlareSolverrResponse {
    status: String,
    solution: Option<FlareSolverrSolution>,
    message: Option<String>,
}

#[derive(Deserialize)]
struct FlareSolverrSolution {
    url: String,
    status: u16,
    headers: serde_json::Value,
    cookies: Vec<FlareSolverrCookie>,
    #[serde(rename = "userAgent")]
    user_agent: String,
}

#[derive(Deserialize)]
struct FlareSolverrCookie {
    name: String,
    value: String,
    domain: Option<String>,
    path: Option<String>,
    expires: Option<f64>,
    #[serde(rename = "httpOnly")]
    http_only: Option<bool>,
    secure: Option<bool>,
    #[serde(rename = "sameSite")]
    same_site: Option<String>,
}

#[derive(Clone)]
pub struct FlareSolverrClient {
    inner: Client,
    config: Arc<RwLock<FlareSolverrConfig>>,
    http: Client,
}

impl FlareSolverrClient {
    pub fn new(config: FlareSolverrConfig) -> anyhow::Result<Self> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .context("failed to create HTTP client for FlareSolverr")?;

        let inner = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.url().scheme() == "magnet" {
                    attempt.stop()
                } else if attempt.previous().len() > 10 {
                    attempt.error("too many redirects")
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .context("failed to create inner HTTP client")?;

        Ok(Self { inner, config: Arc::new(RwLock::new(config)), http })
    }

    pub async fn test(&self) -> anyhow::Result<()> {
        let config = self.config();
        if !config.enabled {
            return Ok(());
        }
        let resp = self
            .http
            .post(&format!("{}/v1", config.url))
            .json(&serde_json::json!({ "cmd": "sessions.list" }))
            .send()
            .await
            .context("failed to connect to FlareSolverr")?;
        if !resp.status().is_success() {
            anyhow::bail!("FlareSolverr returned status: {}", resp.status());
        }
        Ok(())
    }

    pub fn get(&self, url: &str) -> RequestBuilder {
        self.inner.get(url)
    }

    pub fn post(&self, url: &str) -> RequestBuilder {
        self.inner.post(url)
    }

    pub async fn execute_get(&self, url: &str) -> anyhow::Result<Response> {
        if self.is_enabled() {
            self.request_via_flaresolverr("request.get", url, None, None).await
        } else {
            Ok(self.inner.get(url).send().await?)
        }
    }

    async fn request_via_flaresolverr(
        &self,
        cmd: &str,
        url: &str,
        post_data: Option<String>,
        headers: Option<serde_json::Value>,
    ) -> anyhow::Result<Response> {
        let payload = FlareSolverrRequest {
            cmd: cmd.to_owned(),
            url: url.to_owned(),
            max_timeout: Some(60000),
            session: None,
            headers,
            post_data: post_data.clone(),
        };

        let fs_resp: FlareSolverrResponse = self
            .http
            .post(&format!("{}/v1", self.config().url))
            .json(&payload)
            .send()
            .await
            .context("FlareSolverr request failed")?
            .json()
            .await
            .context("invalid FlareSolverr response")?;

        if fs_resp.status != "ok" {
            anyhow::bail!("FlareSolverr error: {}", fs_resp.message.unwrap_or_else(|| "unknown error".into()));
        }

        let solution = fs_resp.solution.context("FlareSolverr returned no solution")?;

        let mut req_builder = self.inner.request(
            if cmd.contains("post") { reqwest::Method::POST } else { reqwest::Method::GET },
            &solution.url,
        );

        if let Some(headers_obj) = solution.headers.as_object() {
            for (k, v) in headers_obj {
                if let Some(v_str) = v.as_str() {
                    req_builder = req_builder.header(k, v_str);
                }
            }
        }

        if !solution.cookies.is_empty() {
            let cookie_header = solution
                .cookies
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; ");
            req_builder = req_builder.header("Cookie", cookie_header);
        }

        req_builder = req_builder.header("User-Agent", solution.user_agent);

        if let Some(data) = post_data {
            req_builder = req_builder.body(data);
        }

        let resp = req_builder.send().await?;
        Ok(resp)
    }

    pub fn inner(&self) -> &Client {
        &self.inner
    }

    pub fn is_enabled(&self) -> bool {
        self.config.read().unwrap().enabled
    }

    pub fn config(&self) -> FlareSolverrConfig {
        self.config.read().unwrap().clone()
    }

    pub fn set_config(&self, config: FlareSolverrConfig) {
        *self.config.write().unwrap() = config;
    }
}

#[async_trait::async_trait]
pub trait FlareSolverrAware: Send + Sync {
    fn set_flaresolverr(&mut self, client: Arc<FlareSolverrClient>);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = FlareSolverrConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.url, "http://localhost:8191");
    }
}
