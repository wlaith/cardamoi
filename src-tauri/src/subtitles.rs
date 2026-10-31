use std::{
    collections::HashSet,
    io::{Read, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, bail};
use librqbit::ManagedTorrent;
use reqwest::{RequestBuilder, StatusCode, header};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::{db, release};

const OPENSUBTITLES: &str = "api.opensubtitles.com";
const USER_AGENT: &str = concat!("cardamoi v", env!("CARGO_PKG_VERSION"));
const SUBDL_API: &str = "https://api.subdl.com/api/v1/subtitles";
const SUBDL_FILES: &str = "https://dl.subdl.com";

pub const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "vtt", "sub", "idx"];
pub const MAX_SUBTITLE_BYTES: u64 = 5 * 1024 * 1024;

pub fn is_subtitle(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| SUBTITLE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub languages: Vec<String>,
    pub auto_fetch: bool,
    pub opensubtitles_key: String,
    pub opensubtitles_user: String,
    /// Never sent to the UI; saving empty keeps the stored one.
    #[serde(default, skip_serializing)]
    pub opensubtitles_password: String,
    #[serde(default, skip_deserializing)]
    pub has_opensubtitles_password: bool,
    pub subdl_key: String,
}

impl Settings {
    pub fn any_provider(&self) -> bool {
        !self.opensubtitles_key.is_empty() || !self.subdl_key.is_empty()
    }

    pub fn has_login(&self) -> bool {
        self.opensubtitles_login().is_some()
    }

    fn opensubtitles_login(&self) -> Option<(&str, &str)> {
        (!self.opensubtitles_user.is_empty() && !self.opensubtitles_password.is_empty())
            .then_some((self.opensubtitles_user.as_str(), self.opensubtitles_password.as_str()))
    }
}

pub fn load_settings(conn: &rusqlite::Connection) -> rusqlite::Result<Settings> {
    let get = |key| db::get_setting(conn, key).map(Option::unwrap_or_default);
    let password = get("opensubtitles_password")?;
    Ok(Settings {
        languages: get("subtitle_languages")?
            .split(',')
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect(),
        auto_fetch: get("auto_subtitles")? == "1",
        opensubtitles_key: get("opensubtitles_key")?,
        opensubtitles_user: get("opensubtitles_user")?,
        has_opensubtitles_password: !password.is_empty(),
        opensubtitles_password: password,
        subdl_key: get("subdl_key")?,
    })
}

pub fn save_settings(conn: &rusqlite::Connection, s: &Settings) -> rusqlite::Result<()> {
    let languages: Vec<String> = s.languages.iter().map(|l| l.trim().to_lowercase()).filter(|l| !l.is_empty()).collect();
    db::set_setting(conn, "subtitle_languages", &languages.join(","))?;
    db::set_setting(conn, "auto_subtitles", if s.auto_fetch { "1" } else { "0" })?;
    db::set_setting(conn, "opensubtitles_key", s.opensubtitles_key.trim())?;
    db::set_setting(conn, "opensubtitles_user", s.opensubtitles_user.trim())?;
    if !s.opensubtitles_password.is_empty() || s.opensubtitles_user.trim().is_empty() {
        db::set_setting(conn, "opensubtitles_password", &s.opensubtitles_password)?;
    }
    db::set_setting(conn, "subdl_key", s.subdl_key.trim())
}

#[derive(Debug, Clone, Default)]
pub struct Query {
    pub tmdb_id: Option<i64>,
    pub tv: bool,
    pub title: String,
    pub year: Option<i32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub languages: Vec<String>,
    pub moviehash: Option<String>,
    pub release_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub provider: String,
    pub key: String,
    pub lang: String,
    pub release: String,
    pub hearing_impaired: bool,
    pub hash_match: bool,
    pub downloads: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    File,
    Release,
    Maybe,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ranked {
    #[serde(flatten)]
    pub found: Found,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quota {
    pub remaining: i64,
    pub resets_at: Option<String>,
}

#[derive(Default)]
pub struct Subtitles {
    login: tokio::sync::Mutex<Option<Login>>,
    quota: Mutex<Option<Quota>>,
    pub attempted: Mutex<HashSet<(i64, i64)>>,
}

#[derive(Clone)]
struct Login {
    token: String,
    host: String,
}

fn opensubtitles(builder: RequestBuilder, key: &str) -> RequestBuilder {
    builder
        .header("Api-Key", key)
        .header(header::USER_AGENT, USER_AGENT)
        .header(header::ACCEPT, "application/json")
}

impl Subtitles {
    pub fn quota(&self) -> Option<Quota> {
        self.quota.lock().unwrap().clone()
    }

    pub async fn sign_out(&self) {
        *self.login.lock().await = None;
    }

    pub async fn sign_in(&self, http: &reqwest::Client, s: &Settings) -> anyhow::Result<Option<(String, String)>> {
        let Some((user, password)) = s.opensubtitles_login() else { return Ok(None) };
        let mut cached = self.login.lock().await;
        if let Some(login) = &*cached {
            return Ok(Some((login.token.clone(), login.host.clone())));
        }
        let response = opensubtitles(http.post(&format!("https://{OPENSUBTITLES}/api/v1/login")), &s.opensubtitles_key)
            .json(&json!({ "username": user, "password": password }))
            .send()
            .await
            .context("couldn't reach OpenSubtitles")?;
        match response.status() {
            StatusCode::UNAUTHORIZED => bail!("OpenSubtitles didn't accept that username and password."),
            StatusCode::FORBIDDEN => bail!("OpenSubtitles didn't accept the API key."),
            status if !status.is_success() => bail!("OpenSubtitles sign-in failed ({status})."),
            _ => {}
        }
        #[derive(Deserialize)]
        struct Reply {
            token: String,
            base_url: Option<String>,
        }
        let reply: Reply = response.json().await?;
        // VIP accounts use their own host.
        let host = reply
            .base_url
            .map(|b| b.trim_start_matches("https://").trim_end_matches('/').to_owned())
            .filter(|b| !b.is_empty() && !b.contains('/'))
            .unwrap_or_else(|| OPENSUBTITLES.to_owned());
        *cached = Some(Login { token: reply.token.clone(), host: host.clone() });
        Ok(Some((reply.token, host)))
    }

    pub async fn search(&self, http: &reqwest::Client, s: &Settings, q: &Query) -> (Vec<Ranked>, Vec<String>) {
        let (os, subdl) = futures::join!(
            async { (!s.opensubtitles_key.is_empty()).then_some(search_opensubtitles(http, s, q).await) },
            async { (!s.subdl_key.is_empty()).then_some(search_subdl(http, s, q).await) },
        );
        let mut found = vec![];
        let mut errors = vec![];
        for (name, outcome) in [("OpenSubtitles", os), ("Subdl", subdl)] {
            match outcome {
                Some(Ok(results)) => found.extend(results),
                Some(Err(e)) => errors.push(format!("{name}: {e:#}")),
                None => {}
            }
        }
        (rank(found, q), errors)
    }

    pub async fn download(
        &self,
        http: &reqwest::Client,
        s: &Settings,
        found: &Found,
        season: Option<u32>,
        episode: Option<u32>,
    ) -> anyhow::Result<(Vec<u8>, String)> {
        match found.provider.as_str() {
            "opensubtitles" => self.download_opensubtitles(http, s, found).await,
            "subdl" => download_subdl(http, found, season, episode).await,
            other => bail!("unknown subtitle provider {other}"),
        }
    }

    async fn download_opensubtitles(
        &self,
        http: &reqwest::Client,
        s: &Settings,
        found: &Found,
    ) -> anyhow::Result<(Vec<u8>, String)> {
        let file_id: u64 = found.key.parse().context("bad OpenSubtitles file id")?;
        let login = self.sign_in(http, s).await?;
        let host = login.as_ref().map_or(OPENSUBTITLES, |(_, host)| host.as_str());
        let mut request = opensubtitles(http.post(&format!("https://{host}/api/v1/download")), &s.opensubtitles_key)
            .json(&json!({ "file_id": file_id, "sub_format": "srt" }));
        if let Some((token, _)) = &login {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.context("couldn't reach OpenSubtitles")?;
        match response.status() {
            StatusCode::UNAUTHORIZED if login.is_none() => {
                bail!("Sign in to OpenSubtitles (free) in Sources to download its subtitles.")
            }
            StatusCode::UNAUTHORIZED => {
                self.sign_out().await;
                bail!("OpenSubtitles signed you out. Try again.")
            }
            StatusCode::NOT_ACCEPTABLE => {
                let body: Value = response.json().await.unwrap_or_default();
                let resets = body["reset_time"].as_str().map(str::to_owned);
                *self.quota.lock().unwrap() = Some(Quota { remaining: 0, resets_at: body["reset_time_utc"].as_str().map(str::to_owned) });
                match resets {
                    Some(r) => bail!("OpenSubtitles daily download limit reached. It resets in {r}."),
                    None => bail!("OpenSubtitles daily download limit reached."),
                }
            }
            StatusCode::TOO_MANY_REQUESTS => bail!("OpenSubtitles is busy. Try again in a few seconds."),
            status if !status.is_success() => bail!("OpenSubtitles download failed ({status})."),
            _ => {}
        }
        #[derive(Deserialize)]
        struct Reply {
            link: String,
            remaining: Option<i64>,
            reset_time_utc: Option<String>,
        }
        let reply: Reply = response.json().await?;
        if let Some(remaining) = reply.remaining {
            *self.quota.lock().unwrap() = Some(Quota { remaining, resets_at: reply.reset_time_utc });
        }
        let bytes = http.get(&reply.link).send().await?.error_for_status()?.bytes().await?;
        Ok((bytes.to_vec(), "srt".into()))
    }
}

async fn search_opensubtitles(http: &reqwest::Client, s: &Settings, q: &Query) -> anyhow::Result<Vec<Found>> {
    let mut languages: Vec<String> = q.languages.iter().map(|l| l.to_lowercase()).collect();
    languages.sort();
    let mut params: Vec<(&str, String)> = vec![("ai_translated", "exclude".into()), ("languages", languages.join(","))];
    match q.tmdb_id {
        Some(id) if q.tv => params.push(("parent_tmdb_id", id.to_string())),
        Some(id) => params.push(("tmdb_id", id.to_string())),
        None => {
            params.push(("query", q.title.to_lowercase()));
            if let Some(year) = q.year.filter(|_| !q.tv) {
                params.push(("year", year.to_string()));
            }
        }
    }
    if let Some(season) = q.season {
        params.push(("season_number", season.to_string()));
    }
    if let Some(episode) = q.episode {
        params.push(("episode_number", episode.to_string()));
    }
    if let Some(hash) = &q.moviehash {
        params.push(("moviehash", hash.clone()));
    }
    // OpenSubtitles redirects unless parameters are sorted.
    params.sort_by_key(|(name, _)| *name);

    let response = opensubtitles(http.get(&format!("https://{OPENSUBTITLES}/api/v1/subtitles")), &s.opensubtitles_key)
        .query(&params)
        .send()
        .await
        .context("couldn't reach OpenSubtitles")?;
    match response.status() {
        StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED => bail!("the API key wasn't accepted"),
        StatusCode::TOO_MANY_REQUESTS => bail!("too many searches; try again in a few seconds"),
        status if !status.is_success() => bail!("search failed ({status})"),
        _ => {}
    }
    let page: Value = response.json().await?;
    Ok(page["data"].as_array().into_iter().flatten().filter_map(|item| opensubtitles_result(&item["attributes"])).collect())
}

fn opensubtitles_result(a: &Value) -> Option<Found> {
    if a["machine_translated"].as_bool() == Some(true) {
        return None;
    }
    let file = a["files"].as_array()?.first()?;
    let release = a["release"].as_str().or(file["file_name"].as_str()).unwrap_or_default();
    Some(Found {
        provider: "opensubtitles".into(),
        key: file["file_id"].as_u64()?.to_string(),
        lang: a["language"].as_str()?.to_lowercase(),
        release: release.to_owned(),
        hearing_impaired: a["hearing_impaired"].as_bool().unwrap_or(false),
        hash_match: a["moviehash_match"].as_bool().unwrap_or(false),
        downloads: a["download_count"].as_u64(),
    })
}

async fn search_subdl(http: &reqwest::Client, s: &Settings, q: &Query) -> anyhow::Result<Vec<Found>> {
    let languages: Vec<String> = q.languages.iter().map(|l| l.to_uppercase()).collect();
    let mut params: Vec<(&str, String)> = vec![
        ("api_key", s.subdl_key.clone()),
        ("languages", languages.join(",")),
        ("subs_per_page", "30".into()),
        ("releases", "1".into()),
        ("type", if q.tv { "tv" } else { "movie" }.into()),
    ];
    match q.tmdb_id {
        Some(id) => params.push(("tmdb_id", id.to_string())),
        None => {
            params.push(("film_name", q.title.clone()));
            if let Some(year) = q.year.filter(|_| !q.tv) {
                params.push(("year", year.to_string()));
            }
        }
    }
    if let Some(season) = q.season {
        params.push(("season_number", season.to_string()));
    }
    if let Some(episode) = q.episode {
        params.push(("episode_number", episode.to_string()));
    }

    let response = http.get(SUBDL_API).query(&params).send().await.context("couldn't reach Subdl")?;
    let status = response.status();
    let page: Value = response.json().await.unwrap_or_default();
    let error = page["error"].as_str().or(page["message"].as_str()).unwrap_or_default().to_owned();
    match status {
        StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED => bail!("the API key wasn't accepted"),
        StatusCode::TOO_MANY_REQUESTS => bail!("too many requests; try again shortly"),
        _ if error.to_lowercase().contains("can't find") => return Ok(vec![]),
        status if !status.is_success() => bail!("search failed ({status})"),
        _ if page["status"].as_bool() == Some(false) => bail!("{}", if error.is_empty() { "search failed" } else { &error }),
        _ => {}
    }
    Ok(page["subtitles"].as_array().into_iter().flatten().filter_map(|item| subdl_result(item, q)).collect())
}

fn subdl_result(item: &Value, q: &Query) -> Option<Found> {
    let url = item["url"].as_str().filter(|u| u.starts_with('/'))?;
    if q.episode.is_some() {
        if item["full_season"].as_bool() == Some(true) {
            return None;
        }
        if let Some(episode) = item["episode"].as_u64() {
            if Some(episode as u32) != q.episode {
                return None;
            }
        }
    }
    let release = item["release_name"]
        .as_str()
        .or_else(|| item["releases"].as_array()?.first()?.as_str())
        .or(item["name"].as_str())
        .unwrap_or_default();
    Some(Found {
        provider: "subdl".into(),
        key: url.to_owned(),
        lang: item["language"].as_str()?.to_lowercase(),
        release: release.to_owned(),
        hearing_impaired: item["hi"].as_bool().or_else(|| item["hi"].as_u64().map(|n| n != 0)).unwrap_or(false),
        hash_match: false,
        downloads: None,
    })
}

async fn download_subdl(
    http: &reqwest::Client,
    found: &Found,
    season: Option<u32>,
    episode: Option<u32>,
) -> anyhow::Result<(Vec<u8>, String)> {
    anyhow::ensure!(found.key.starts_with('/'), "bad Subdl download path");
    let response = http.get(&format!("{SUBDL_FILES}{}", found.key)).send().await.context("couldn't reach Subdl")?;
    if response.status() == StatusCode::TOO_MANY_REQUESTS {
        bail!("Subdl daily download limit reached.");
    }
    let bytes = response.error_for_status()?.bytes().await?;
    unpack(&bytes, season, episode)
}

fn unpack(bytes: &[u8], season: Option<u32>, episode: Option<u32>) -> anyhow::Result<(Vec<u8>, String)> {
    let Ok(mut archive) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else {
        return Ok((bytes.to_vec(), "srt".into()));
    };
    let names: Vec<String> = archive
        .file_names()
        .filter(|n| !n.ends_with('/') && !n.starts_with("__MACOSX/") && !n.rsplit('/').next().unwrap_or(n).starts_with("._"))
        .filter(|n| {
            let ext = Path::new(n).extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
            matches!(ext.as_str(), "srt" | "ass" | "ssa" | "vtt")
        })
        .map(str::to_owned)
        .collect();
    let for_episode = names.iter().find(|n| {
        let r = release::parse(Path::new(n).file_stem().and_then(|s| s.to_str()).unwrap_or_default());
        episode.is_some() && r.season == season && r.episode == episode
    });
    let name = for_episode.or(names.first()).context("the download had no subtitle file in it")?.clone();
    let mut file = archive.by_name(&name)?;
    let mut text = vec![];
    file.read_to_end(&mut text)?;
    let ext = Path::new(&name).extension().and_then(|e| e.to_str()).unwrap_or("srt").to_ascii_lowercase();
    Ok((text, ext))
}

fn release_group(name: &str) -> Option<String> {
    let name = strip_extension(name);
    let group = name.rsplit_once('-')?.1.trim();
    (2..=20).contains(&group.len()).then(|| group.to_lowercase()).filter(|g| g.chars().all(char::is_alphanumeric))
}

fn strip_extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, ext)) if (2..=4).contains(&ext.len()) && ext.chars().all(char::is_alphanumeric) => stem,
        _ => name,
    }
}

pub fn confidence(found: &Found, release_name: &str) -> Confidence {
    if found.hash_match {
        return Confidence::File;
    }
    let (ours, theirs) = (strip_extension(release_name), strip_extension(&found.release));
    if !ours.is_empty() && release::normalize(ours) == release::normalize(theirs) {
        return Confidence::Release;
    }
    let same_group = release_group(ours).is_some_and(|g| release_group(theirs) == Some(g));
    let (a, b) = (release::parse(ours).resolution, release::parse(theirs).resolution);
    if same_group && (a.is_none() || b.is_none() || a == b) { Confidence::Release } else { Confidence::Maybe }
}

pub fn rank(found: Vec<Found>, q: &Query) -> Vec<Ranked> {
    let mut seen = HashSet::new();
    let mut ranked: Vec<Ranked> = found
        .into_iter()
        .filter(|f| seen.insert((f.provider.clone(), f.key.clone())))
        .map(|found| Ranked { confidence: confidence(&found, &q.release_name), found })
        .collect();
    let language = |lang: &str| q.languages.iter().position(|l| lang.starts_with(l.as_str())).unwrap_or(usize::MAX);
    ranked.sort_by_key(|r| {
        (r.confidence, language(&r.found.lang), r.found.hearing_impaired, std::cmp::Reverse(r.found.downloads))
    });
    ranked
}

/// OpenSubtitles hash: size plus u64 LE sum of the first and last 64 KiB.
pub fn moviehash(size: u64, head: &[u8], tail: &[u8]) -> String {
    let hash = head
        .chunks_exact(8)
        .chain(tail.chunks_exact(8))
        .fold(size, |h, chunk| h.wrapping_add(u64::from_le_bytes(chunk.try_into().unwrap())));
    format!("{hash:016x}")
}

pub fn moviehash_of_file(path: &Path) -> Option<String> {
    use std::io::Seek;
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let chunk = len.min(64 * 1024) as usize;
    let mut head = vec![0; chunk];
    file.read_exact(&mut head).ok()?;
    file.seek(SeekFrom::Start(len - chunk as u64)).ok()?;
    let mut tail = vec![0; chunk];
    file.read_exact(&mut tail).ok()?;
    Some(moviehash(len, &head, &tail))
}

pub async fn moviehash_of(handle: Arc<ManagedTorrent>, index: usize, len: u64, wait: Duration) -> Option<String> {
    let chunk = len.min(64 * 1024) as usize;
    let read = async {
        let mut stream = handle.stream(index).await?;
        let mut head = vec![0; chunk];
        stream.read_exact(&mut head).await?;
        stream.seek(SeekFrom::Start(len - chunk as u64)).await?;
        let mut tail = vec![0; chunk];
        stream.read_exact(&mut tail).await?;
        anyhow::Ok(moviehash(len, &head, &tail))
    };
    tokio::time::timeout(wait, read).await.ok()?.ok()
}

pub fn save_beside(video: &Path, lang: &str, ext: &str, text: &[u8]) -> std::io::Result<PathBuf> {
    let dir = video.parent().unwrap_or(Path::new("."));
    let stem = video.file_stem().and_then(|s| s.to_str()).unwrap_or("subtitle");
    let lang: String = lang.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    let ext = if SUBTITLE_EXTENSIONS.contains(&ext) { ext } else { "srt" };
    std::fs::create_dir_all(dir)?;
    for n in 1.. {
        let name = if n == 1 { format!("{stem}.{lang}.{ext}") } else { format!("{stem}.{lang}.{n}.{ext}") };
        let path = dir.join(name);
        if !path.exists() {
            std::fs::write(&path, text)?;
            return Ok(path);
        }
    }
    unreachable!()
}

pub fn loads_by_itself(subtitle: &Path, video: &Path) -> bool {
    let stem = video.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    subtitle.parent() == video.parent()
        && subtitle.file_name().and_then(|n| n.to_str()).is_some_and(|n| !stem.is_empty() && n.starts_with(stem))
}

pub fn guess_language(path: &Path) -> Option<&'static str> {
    const NAMES: &[(&str, &[&str])] = &[
        ("en", &["english", "eng"]),
        ("ar", &["arabic", "ara"]),
        ("fr", &["french", "fre", "fra"]),
        ("es", &["spanish", "spa"]),
        ("de", &["german", "ger", "deu"]),
        ("it", &["italian", "ita"]),
        ("pt", &["portuguese", "por"]),
        ("ru", &["russian", "rus"]),
        ("tr", &["turkish", "tur"]),
        ("nl", &["dutch", "dut", "nld"]),
        ("zh", &["chinese", "chi", "zho"]),
        ("ja", &["japanese", "jpn"]),
        ("ko", &["korean", "kor"]),
        ("fa", &["persian", "farsi", "per", "fas"]),
        ("he", &["hebrew", "heb"]),
        ("hi", &["hindi", "hin"]),
        ("id", &["indonesian", "ind"]),
        ("pl", &["polish", "pol"]),
        ("sv", &["swedish", "swe"]),
        ("el", &["greek", "gre", "ell"]),
    ];
    let stem = path.file_stem()?.to_str()?.to_lowercase();
    let words: Vec<&str> = stem.split(|c: char| !c.is_ascii_alphabetic()).filter(|w| !w.is_empty()).collect();
    // Two-letter codes only as the last word ("it", "he" are words).
    if let Some(last) = words.last() {
        if let Some((code, _)) = NAMES.iter().find(|(code, _)| code == last) {
            return Some(code);
        }
    }
    words.iter().rev().find_map(|w| NAMES.iter().find(|(_, names)| names.contains(w)).map(|(code, _)| *code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_like_opensubtitles() {
        let zeros = vec![0u8; 64 * 1024];
        assert_eq!(moviehash(131072, &zeros, &zeros), "0000000000020000");
        let mut head = zeros.clone();
        head[0] = 1;
        head[8] = 2;
        assert_eq!(moviehash(131072, &head, &zeros), "0000000000020003");
        let ones = vec![0xffu8; 8];
        assert_eq!(moviehash(1, &ones, &[]), "0000000000000000");
    }

    fn found(release: &str, hash: bool) -> Found {
        Found {
            provider: "opensubtitles".into(),
            key: release.into(),
            lang: "en".into(),
            release: release.into(),
            hearing_impaired: false,
            hash_match: hash,
            downloads: Some(10),
        }
    }

    #[test]
    fn confidence_levels() {
        let ours = "Severance.S02E03.1080p.WEB.h264-ETHEL.mkv";
        assert_eq!(confidence(&found("anything", true), ours), Confidence::File);
        assert_eq!(confidence(&found("Severance.S02E03.1080p.WEB.h264-ETHEL", false), ours), Confidence::Release);
        assert_eq!(confidence(&found("Severance.S02E03.1080p.WEB-ethel", false), ours), Confidence::Release);
        assert_eq!(confidence(&found("Severance.S02E03.720p.WEB-ETHEL", false), ours), Confidence::Maybe);
        assert_eq!(confidence(&found("Severance.S02E03.720p.HDTV", false), ours), Confidence::Maybe);
    }

    #[test]
    fn ranks_sync_then_language_then_popularity() {
        let q = Query { languages: vec!["ar".into(), "en".into()], release_name: "Dune.2021.1080p-GRP.mkv".into(), ..Default::default() };
        let mut popular = found("Dune.2021.720p", false);
        popular.downloads = Some(900);
        let mut arabic = found("Dune 2021", false);
        arabic.lang = "ar".into();
        let mut duplicate = found("Dune.2021.1080p-GRP", false);
        duplicate.key = "same".into();
        let mut same = found("Dune.2021.1080p-GRP", false);
        same.key = "same".into();
        let mut other = found("Dune.2021.720p", false);
        other.key = "other".into();
        let ranked = rank(vec![popular, other, arabic, duplicate, same, found("x", true)], &q);
        let order: Vec<_> = ranked.iter().map(|r| (r.confidence, r.found.lang.as_str(), r.found.downloads)).collect();
        assert_eq!(
            order,
            [
                (Confidence::File, "en", Some(10)),
                (Confidence::Release, "en", Some(10)),
                (Confidence::Maybe, "ar", Some(10)),
                (Confidence::Maybe, "en", Some(900)),
                (Confidence::Maybe, "en", Some(10)),
            ]
        );
    }

    #[test]
    fn reads_subdl_results() {
        let q = Query { episode: Some(3), season: Some(2), ..Default::default() };
        let item = json!({ "url": "/subtitle/1-2.zip", "language": "EN", "release_name": "Show.S02E03.WEB", "hi": 1, "episode": 3 });
        let f = subdl_result(&item, &q).unwrap();
        assert_eq!((f.lang.as_str(), f.release.as_str(), f.hearing_impaired), ("en", "Show.S02E03.WEB", true));
        assert!(subdl_result(&json!({ "url": "/s.zip", "language": "EN", "episode": 4 }), &q).is_none());
        assert!(subdl_result(&json!({ "url": "/s.zip", "language": "EN", "full_season": true }), &q).is_none());
        assert!(subdl_result(&json!({ "url": "https://elsewhere/s.zip", "language": "EN" }), &q).is_none());
    }

    #[test]
    fn reads_opensubtitles_results() {
        let a = json!({ "language": "pt-BR", "release": "Dune.2021", "moviehash_match": true, "download_count": 5,
                        "files": [{ "file_id": 42, "file_name": "dune.srt" }] });
        let f = opensubtitles_result(&a).unwrap();
        assert_eq!((f.key.as_str(), f.lang.as_str(), f.hash_match, f.downloads), ("42", "pt-br", true, Some(5)));
        assert!(opensubtitles_result(&json!({ "language": "en", "files": [] })).is_none());
        assert!(opensubtitles_result(&json!({ "language": "en", "machine_translated": true, "files": [{ "file_id": 1 }] })).is_none());
    }

    #[test]
    fn unpacks_the_episode_from_a_zip() {
        let mut bytes = std::io::Cursor::new(vec![]);
        {
            let mut zip = zip::ZipWriter::new(&mut bytes);
            let opts = zip::write::SimpleFileOptions::default();
            for (name, text) in [("__MACOSX/._a.srt", "junk"), ("Show.S02E02.srt", "two"), ("Show.S02E03.srt", "three"), ("readme.txt", "hi")] {
                zip.start_file(name, opts).unwrap();
                std::io::Write::write_all(&mut zip, text.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        let bytes = bytes.into_inner();
        assert_eq!(unpack(&bytes, Some(2), Some(3)).unwrap(), (b"three".to_vec(), "srt".into()));
        assert_eq!(unpack(&bytes, None, None).unwrap().0, b"two");
        assert_eq!(unpack(b"1\n00:00:01,000 --> 00:00:02,000\nHi\n", None, None).unwrap().1, "srt");
    }

    #[test]
    fn saves_beside_the_video_without_overwriting() {
        let dir = std::env::temp_dir().join(format!("cardamoi-subs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let video = dir.join("Dune.2021.mkv");
        let first = save_beside(&video, "en", "srt", b"a").unwrap();
        let second = save_beside(&video, "en", "srt", b"b").unwrap();
        assert_eq!(first.file_name().unwrap(), "Dune.2021.en.srt");
        assert_eq!(second.file_name().unwrap(), "Dune.2021.en.2.srt");
        assert_eq!(save_beside(&video, "../x", "exe", b"c").unwrap().file_name().unwrap(), "Dune.2021.x.srt");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn guesses_languages_from_file_names() {
        assert_eq!(guess_language(Path::new("Subs/2_English.srt")), Some("en"));
        assert_eq!(guess_language(Path::new("Movie.2020.ara.srt")), Some("ar"));
        assert_eq!(guess_language(Path::new("Movie.2020.en.srt")), Some("en"));
        assert_eq!(guess_language(Path::new("It.Follows.2014.srt")), None);
        assert_eq!(guess_language(Path::new("movie.srt")), None);
    }
}
