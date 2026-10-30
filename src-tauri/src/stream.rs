use std::{io::SeekFrom, sync::Arc};

use anyhow::Context;
use axum::{
    Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use librqbit::Session;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

pub struct StreamServer {
    port: u16,
    /// New each launch so other local programs can't read the library.
    token: String,
}

impl StreamServer {
    pub fn url(&self, info_hash: &str, file_index: usize) -> String {
        format!("http://127.0.0.1:{}/v/{info_hash}/{file_index}?t={}", self.port, self.token)
    }
}

#[derive(Clone)]
struct Shared {
    session: Arc<Session>,
    token: Arc<str>,
}

pub async fn start(session: Arc<Session>) -> anyhow::Result<StreamServer> {
    let token = uuid::Uuid::new_v4().simple().to_string();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let app = Router::new()
        .route("/v/{hash}/{file}", get(video))
        .with_state(Shared { session, token: token.clone().into() });
    tauri::async_runtime::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("stream server stopped: {e:#}");
        }
    });
    Ok(StreamServer { port, token })
}

#[derive(Deserialize)]
struct Auth {
    t: String,
}

async fn video(
    State(shared): State<Shared>,
    Path((hash, file)): Path<(String, usize)>,
    Query(auth): Query<Auth>,
    headers: HeaderMap,
) -> Response {
    if auth.t != *shared.token {
        return StatusCode::FORBIDDEN.into_response();
    }
    let range = headers.get(header::RANGE).and_then(|r| r.to_str().ok());
    match serve(&shared.session, &hash, file, range).await {
        Ok(response) => response,
        Err(e) => (StatusCode::NOT_FOUND, format!("{e:#}")).into_response(),
    }
}

async fn serve(session: &Session, hash: &str, file: usize, range: Option<&str>) -> anyhow::Result<Response> {
    let handle = crate::find_torrent(session, hash).context("no such torrent")?;
    let (len, name) = handle
        .with_metadata(|m| m.file_infos.get(file).map(|f| (f.len, f.relative_filename.clone())))?
        .context("no such file")?;

    let partial = match range {
        Some(range) => match parse_range(range, len) {
            Some(r) => Some(r),
            None => {
                return Ok((StatusCode::RANGE_NOT_SATISFIABLE, [(header::CONTENT_RANGE, format!("bytes */{len}"))])
                    .into_response());
            }
        },
        None => None,
    };
    let (start, end) = partial.unwrap_or((0, len.saturating_sub(1)));
    let count = if len == 0 { 0 } else { end + 1 - start };

    let mut stream = handle.stream(file).await?;
    stream.seek(SeekFrom::Start(start)).await?;
    let body = Body::from_stream(ReaderStream::new(stream.take(count)));

    let mut response = Response::builder()
        .status(if partial.is_some() { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK })
        .header(header::CONTENT_TYPE, content_type(&name))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, count);
    if partial.is_some() {
        response = response.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
    }
    Ok(response.body(body)?)
}

fn parse_range(header: &str, len: u64) -> Option<(u64, u64)> {
    if len == 0 {
        return None;
    }
    let spec = header.strip_prefix("bytes=")?.split(',').next()?.trim();
    let (from, to) = spec.split_once('-')?;
    let (start, end) = if from.is_empty() {
        let suffix: u64 = to.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        (len.saturating_sub(suffix), len - 1)
    } else {
        let start: u64 = from.parse().ok()?;
        let end = if to.is_empty() { len - 1 } else { to.parse::<u64>().ok()?.min(len - 1) };
        (start, end)
    };
    (start <= end && start < len).then_some((start, end))
}

fn content_type(path: &std::path::Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range("bytes=0-", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_range("bytes=90-500", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=-500", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=5-9, 20-30", 100), Some((5, 9)));
    }

    #[test]
    fn unsatisfiable_ranges() {
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=20-10", 100), None);
        assert_eq!(parse_range("bytes=-0", 100), None);
        assert_eq!(parse_range("items=0-1", 100), None);
        assert_eq!(parse_range("bytes=0-", 0), None);
    }
}
