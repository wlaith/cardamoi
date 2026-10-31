mod db;
mod flaresolverr;
mod local;
mod player;
mod prowlarr;
mod release;
mod sources;
mod stream;
mod subtitles;
mod tmdb;

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::Context;
use librqbit::{
    AddTorrent, AddTorrentOptions, Magnet, ManagedTorrent, Session, SessionOptions,
    SessionPersistenceConfig, TorrentStatsState, api::TorrentIdOrHash, dht::Id20,
};
use player::{PlayError, Player};
use serde::{Deserialize, Serialize};
use prowlarr::{Prowlarr, ProwlarrSource};
use sources::{InternetArchive, SearchQuery, SearchResult, Source, Torznab};
use tmdb::Tmdb;
use tauri::{
    menu::{Menu, MenuItemBuilder, SubmenuBuilder},
    AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_opener::OpenerExt;
use flaresolverr::{FlareSolverrClient, FlareSolverrConfig};

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "m4v", "mkv", "webm", "mov", "avi", "ogv", "mpg", "mpeg", "wmv"];

struct AppState {
    session: Arc<Session>,
    db: Mutex<rusqlite::Connection>,
    http: Arc<FlareSolverrClient>,
    download_dir: PathBuf,
    prowlarr: Arc<Prowlarr>,
    stream: stream::StreamServer,
    player: Player,
    subtitles: subtitles::Subtitles,
    resolving: Mutex<HashMap<i64, Resolving>>,
    folder_sizes: Mutex<HashMap<PathBuf, u64>>,
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

#[derive(Serialize)]
struct Found {
    #[serde(flatten)]
    result: SearchResult,
    release: release::Release,
    matches: bool,
}

#[derive(Serialize)]
struct SearchResponse {
    results: Vec<Found>,
    errors: Vec<String>,
}

fn active_sources(state: &AppState) -> anyhow::Result<Vec<Box<dyn Source>>> {
    let mut sources: Vec<Box<dyn Source>> = vec![
        Box::new(InternetArchive { http: state.http.clone() }),
        Box::new(ProwlarrSource { prowlarr: state.prowlarr.clone(), http: state.http.clone() }),
    ];
    for row in db::list_sources(&state.db.lock().unwrap())? {
        if row.enabled && row.kind == "torznab" {
            sources.push(Box::new(Torznab {
                http: state.http.clone(),
                name: row.name,
                url: row.url,
                api_key: row.api_key,
            }));
        }
    }
    Ok(sources)
}

#[tauri::command]
async fn search(state: State<'_, AppState>, query: SearchQuery) -> CmdResult<SearchResponse> {
    let sources = active_sources(&state).map_err(err)?;
    let outcomes = futures::future::join_all(sources.iter().map(|s| s.search(&query))).await;
    let mut response = SearchResponse { results: vec![], errors: vec![] };
    for (source, outcome) in sources.iter().zip(outcomes) {
        match outcome {
            Ok(results) => response.results.extend(results.into_iter().map(|result| {
                let mut parsed = release::parse(&result.title);
                parsed.year = parsed.year.or(result.year);
                let matches = query.match_title.as_deref().is_none_or(|title| {
                    release::matches(&parsed, title, query.year, query.season, query.episode)
                });
                Found { result, release: parsed, matches }
            })),
            Err(e) => response.errors.push(format!("{}: {e:#}", source.name())),
        }
    }
    response.results.sort_by_key(|f| (!f.matches, std::cmp::Reverse(f.result.seeders)));
    Ok(response)
}

#[tauri::command]
async fn catalog_search(state: State<'_, AppState>, text: String) -> CmdResult<Vec<tmdb::Title>> {
    Tmdb { http: state.http.clone() }.search(text.trim()).await.map_err(err)
}

#[tauri::command]
async fn title_details(state: State<'_, AppState>, kind: tmdb::Kind, id: u64) -> CmdResult<tmdb::Details> {
    Tmdb { http: state.http.clone() }.details(kind, id).await.map_err(err)
}

#[tauri::command]
async fn season_episodes(state: State<'_, AppState>, id: u64, season: u32) -> CmdResult<Vec<tmdb::Episode>> {
    Tmdb { http: state.http.clone() }.episodes(id, season).await.map_err(err)
}

#[tauri::command]
fn prowlarr_status(state: State<'_, AppState>) -> prowlarr::Status {
    state.prowlarr.status()
}

#[tauri::command]
fn open_prowlarr(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(window) = app.get_webview_window("prowlarr") {
        return window.set_focus().map_err(err);
    }
    let url = state.prowlarr.base_url.parse().map_err(err)?;
    WebviewWindowBuilder::new(&app, "prowlarr", WebviewUrl::External(url))
        .title("Manage indexers")
        .inner_size(1100.0, 800.0)
        .build()
        .map_err(err)?;
    Ok(())
}

#[tauri::command]
fn list_sources(state: State<'_, AppState>) -> CmdResult<Vec<db::SourceRow>> {
    db::list_sources(&state.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
async fn add_source(state: State<'_, AppState>, name: String, url: String, api_key: String) -> CmdResult<()> {
    let source = Torznab {
        http: state.http.clone(),
        name: name.trim().to_owned(),
        url: url.trim().to_owned(),
        api_key: api_key.trim().to_owned(),
    };
    if source.name.is_empty() || source.url.is_empty() {
        return Err("A source needs a name and a URL.".into());
    }
    source.test().await.map_err(|e| format!("Couldn't connect: {e:#}"))?;
    db::insert_source(&state.db.lock().unwrap(), &source.name, &source.url, &source.api_key).map_err(err)
}

#[tauri::command]
fn set_source_enabled(state: State<'_, AppState>, id: i64, enabled: bool) -> CmdResult<()> {
    db::set_source_enabled(&state.db.lock().unwrap(), id, enabled).map_err(err)
}

#[tauri::command]
fn remove_source(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    db::delete_source(&state.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command]
fn get_flaresolverr_config(state: State<'_, AppState>) -> CmdResult<FlareSolverrConfig> {
    let config = db::get_flaresolverr_config(&state.db.lock().unwrap()).map_err(err)?;
    Ok(config)
}

#[tauri::command]
async fn set_flaresolverr_config(state: State<'_, AppState>, config: FlareSolverrConfig) -> CmdResult<()> {
    db::set_flaresolverr_config(&state.db.lock().unwrap(), &config).map_err(err)?;
    state.http.set_config(config);
    Ok(())
}

#[tauri::command]
async fn test_flaresolverr(state: State<'_, AppState>) -> CmdResult<()> {
    state.http.test().await.map_err(err)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddRequest {
    title: String,
    year: Option<i32>,
    source: String,
    torrent_url: String,
    poster: Option<String>,
    #[serde(default)]
    single_video: bool,
    tmdb_id: Option<i64>,
    kind: Option<tmdb::Kind>,
    season: Option<u32>,
    episode: Option<u32>,
}

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn best_video<'a>(files: impl Iterator<Item = (usize, &'a Path, u64)>) -> Option<usize> {
    let videos: Vec<_> = files.filter(|(_, p, _)| is_video(p)).collect();
    let is_mp4 = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mp4"));
    let pool: Vec<_> = if videos.iter().any(|(_, p, _)| is_mp4(p)) {
        videos.into_iter().filter(|(_, p, _)| is_mp4(p)).collect()
    } else {
        videos
    };
    pool.into_iter().max_by_key(|(_, _, len)| *len).map(|(i, _, _)| i)
}

fn is_sample(path: &Path, len: u64, largest: u64) -> bool {
    let named = path.iter().any(|part| {
        part.to_string_lossy().to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|w| w == "sample")
    });
    named && len < largest / 3
}

fn discover_videos(item: &db::Item, files: &[(usize, PathBuf, u64)]) -> Option<(Vec<db::NewVideo>, i64)> {
    let videos: Vec<_> = files.iter().filter(|(_, p, _)| is_video(p)).collect();
    let largest = videos.iter().map(|(_, _, len)| *len).max()?;
    let videos: Vec<_> = videos.into_iter().filter(|(_, p, len)| !is_sample(p, *len, largest)).collect();
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let new = |(i, path, len): &(usize, PathBuf, u64), season, episode| db::NewVideo {
        file_index: *i as i64,
        path: path.to_string_lossy().into_owned(),
        len: *len,
        season,
        episode,
    };

    let mut episodes: BTreeMap<(Option<u32>, u32), &(usize, PathBuf, u64)> = BTreeMap::new();
    if item.kind.as_deref() != Some("movie") {
        for file in &videos {
            let parsed = release::parse(&name(&file.1));
            if let Some(episode) = parsed.episode {
                let key = (parsed.season.or(item.season), episode);
                if episodes.get(&key).is_none_or(|kept| file.2 > kept.2) {
                    episodes.insert(key, file);
                }
            }
        }
    }
    if episodes.len() >= 2 {
        let main = episodes.values().max_by_key(|f| f.2)?.0 as i64;
        let list = episodes.into_iter().map(|((s, e), file)| new(file, s, Some(e))).collect();
        return Some((list, main));
    }
    let main = best_video(videos.iter().map(|(i, p, len)| (*i, p.as_path(), *len)))?;
    let file = videos.iter().find(|(i, _, _)| *i == main)?;
    let parsed = release::parse(&name(&file.1));
    let (season, episode) = match item.kind.as_deref() {
        Some("movie") => (None, None),
        _ => (item.season.or(parsed.season), item.episode.or(parsed.episode)),
    };
    Some((vec![new(file, season, episode)], main as i64))
}

fn torrent_files(handle: &ManagedTorrent) -> anyhow::Result<Vec<(usize, PathBuf, u64)>> {
    let only_files = handle.only_files();
    handle.with_metadata(|m| {
        m.file_infos
            .iter()
            .enumerate()
            .filter(|(i, _)| only_files.as_ref().is_none_or(|only| only.contains(i)))
            .map(|(i, f)| (i, f.relative_filename.clone(), f.len))
            .collect()
    })
}

#[derive(Clone)]
enum Files {
    Torrent(Arc<ManagedTorrent>),
    Local(PathBuf),
}

impl Files {
    fn of(state: &AppState, item: &db::Item) -> Option<Files> {
        match &item.local_path {
            Some(path) => Some(Files::Local(Path::new(path).parent()?.to_path_buf())),
            None => find_torrent(&state.session, &item.info_hash).map(Files::Torrent),
        }
    }

    fn folder(&self) -> PathBuf {
        match self {
            Files::Torrent(handle) => handle.output_folder().to_path_buf(),
            Files::Local(dir) => dir.clone(),
        }
    }

    fn progress(&self, videos: &[db::Video]) -> Vec<u64> {
        match self {
            Files::Torrent(handle) => handle.stats().file_progress,
            Files::Local(dir) => {
                let len = videos.iter().map(|v| v.file_index as usize + 1).max().unwrap_or(0);
                let mut progress = vec![0; len];
                for v in videos.iter().filter(|v| dir.join(&v.path).exists()) {
                    progress[v.file_index as usize] = v.len;
                }
                progress
            }
        }
    }

    fn torrent(&self) -> Option<&Arc<ManagedTorrent>> {
        match self {
            Files::Torrent(handle) => Some(handle),
            Files::Local(_) => None,
        }
    }
}

fn ensure_videos(state: &AppState, item: &db::Item, files: &Files) -> anyhow::Result<Vec<db::Video>> {
    let known = db::videos_for(&state.db.lock().unwrap(), item.id)?;
    let Files::Torrent(handle) = files else { return Ok(known) };
    if !known.is_empty() {
        return Ok(known);
    }
    // Fails until magnet metadata arrives.
    let files = torrent_files(handle)?;
    let Some((videos, main)) = discover_videos(item, &files) else { return Ok(vec![]) };
    let db = state.db.lock().unwrap();
    db::save_videos(&db, item.id, &videos, main)?;
    Ok(db::videos_for(&db, item.id)?)
}

fn next_up(videos: Vec<db::Video>) -> Option<db::Video> {
    let started = videos
        .iter()
        .enumerate()
        .filter(|(_, v)| player::resume_from(v.position_secs, v.duration_secs).is_some())
        .max_by_key(|(_, v)| v.last_played_at)
        .map(|(i, _)| i);
    let pick = started.or_else(|| videos.iter().position(|v| !v.watched)).unwrap_or(0);
    videos.into_iter().nth(pick)
}

fn resolve(state: &AppState, id: i64, file_index: Option<i64>) -> anyhow::Result<(db::Item, Files, db::Video)> {
    let item = db::get(&state.db.lock().unwrap(), id)?;
    let files = Files::of(state, &item).context("the torrent is no longer in the download engine")?;
    let videos = ensure_videos(state, &item, &files)?;
    let video = match file_index {
        Some(index) => videos.into_iter().find(|v| v.file_index == index),
        None => next_up(videos),
    };
    Ok((item, files, video.context("no video file in this download")?))
}
#[tauri::command]
async fn add_download(app: AppHandle, state: State<'_, AppState>, req: AddRequest) -> CmdResult<()> {
    add_download_inner(&app, &state, req).await.map_err(err)
}

async fn add_download_inner(app: &AppHandle, state: &AppState, req: AddRequest) -> anyhow::Result<()> {
    let mut url = req.torrent_url.trim().to_owned();
    let mut torrent_bytes = None;
    if url.starts_with("http://") || url.starts_with("https://") {
        let resp = state.http.get(&url).send().await?;
        // Indexer links often redirect to a magnet.
        let magnet = resp
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|l| l.to_str().ok())
            .filter(|l| l.starts_with("magnet:"))
            .map(str::to_owned);
        match magnet {
            Some(magnet) => url = magnet,
            None => {
                let bytes = resp
                    .error_for_status()
                    .context("could not fetch the .torrent file")?
                    .bytes()
                    .await?;
                torrent_bytes = Some(bytes);
            }
        }
    }

    match torrent_bytes {
        Some(bytes) => {
            let handle = add_torrent(state, AddTorrent::from_bytes(bytes), req.single_video).await?;
            let id = record(state, &req, &handle.info_hash().as_string(), None)?;
            identify(state, id, release::parse(&download_name(&req.title, &handle))).await;
        }
        None => {
            let magnet = Magnet::parse(&url).context("that isn't a magnet link or a link to a .torrent file")?;
            let hash = magnet.as_id20().context("this magnet link has no BitTorrent v1 hash")?.as_string();
            let id = record(state, &req, &hash, Some(&url))?;
            if find_torrent(&state.session, &hash).is_none() {
                spawn_resolve(app, id, url, req.single_video);
            }
        }
    }
    Ok(())
}

async fn add_torrent(state: &AppState, add: AddTorrent<'_>, single_video: bool) -> anyhow::Result<Arc<ManagedTorrent>> {
    let opts = AddTorrentOptions { overwrite: true, paused: single_video, ..Default::default() };
    let handle = state
        .session
        .add_torrent(add, Some(opts))
        .await?
        .into_handle()
        .context("torrent was not added")?;
    if single_video {
        let files = torrent_files(&handle)?;
        let pick = best_video(files.iter().map(|(i, p, len)| (*i, p.as_path(), *len)))
            .context("this torrent has no video files")?;
        let keep: HashSet<usize> = files
            .iter()
            .filter(|(i, p, len)| *i == pick || (subtitles::is_subtitle(p) && *len <= subtitles::MAX_SUBTITLE_BYTES))
            .map(|(i, _, _)| *i)
            .collect();
        state.session.update_only_files(&handle, &keep).await?;
        state.session.unpause(&handle).await?;
    }
    Ok(handle)
}

fn record(state: &AppState, req: &AddRequest, info_hash: &str, magnet: Option<&str>) -> anyhow::Result<i64> {
    let kind = req.kind.map(|k| match k {
        tmdb::Kind::Movie => "movie",
        tmdb::Kind::Tv => "tv",
    });
    let id = db::insert(
        &state.db.lock().unwrap(),
        &db::NewItem {
            title: req.title.trim(),
            year: req.year,
            source: &req.source,
            info_hash,
            poster: req.poster.as_deref(),
            tmdb_id: req.tmdb_id,
            kind,
            season: req.season,
            episode: req.episode,
            torrent_url: magnet,
            single_video: req.single_video,
            local_path: None,
        },
    )?;
    Ok(id)
}

async fn identify(state: &AppState, id: i64, parsed: release::Release) {
    let Ok(item) = db::get(&state.db.lock().unwrap(), id) else { return };
    if item.tmdb_id.is_some() {
        return;
    }
    if parsed.title.is_empty() {
        return;
    }
    let tv = parsed.season.is_some();
    let Ok(Some(found)) = (Tmdb { http: state.http.clone() }).best_match(&parsed.title, parsed.year, tv).await else {
        return;
    };
    let kind = match found.kind {
        tmdb::Kind::Movie => "movie",
        tmdb::Kind::Tv => "tv",
    };
    let matched = db::Match {
        title: &found.title,
        year: found.year,
        poster: found.poster.as_deref(),
        tmdb_id: found.id as i64,
        kind,
        season: parsed.season,
        episode: parsed.episode,
    };
    if let Err(e) = db::set_match(&state.db.lock().unwrap(), id, &matched) {
        eprintln!("couldn't save what download {id} is: {e}");
    }
}

fn download_name(title: &str, handle: &ManagedTorrent) -> String {
    if title.trim().is_empty() { handle.name().unwrap_or_default() } else { title.to_owned() }
}

struct Resolving {
    since: std::time::Instant,
    error: Option<String>,
    task: Option<tauri::async_runtime::JoinHandle<()>>,
}

fn spawn_resolve(app: &AppHandle, id: i64, magnet: String, single_video: bool) {
    let state = app.state::<AppState>();
    let mut resolving = state.resolving.lock().unwrap();
    if resolving.get(&id).is_some_and(|r| r.error.is_none()) {
        return;
    }
    let app = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let outcome = add_torrent(&state, AddTorrent::from_url(magnet), single_video).await;
        // Deleted while waiting for peers: drop the torrent.
        let wanted = db::get(&state.db.lock().unwrap(), id).is_ok();
        match outcome {
            Ok(handle) if wanted => {
                state.resolving.lock().unwrap().remove(&id);
                let title = db::get(&state.db.lock().unwrap(), id).map(|i| i.title).unwrap_or_default();
                identify(&state, id, release::parse(&download_name(&title, &handle))).await;
            }
            Ok(handle) => {
                let _ = state.session.delete(TorrentIdOrHash::Hash(handle.info_hash()), true).await;
                state.resolving.lock().unwrap().remove(&id);
            }
            Err(e) => match state.resolving.lock().unwrap().get_mut(&id) {
                Some(r) if wanted => {
                    r.error = Some(format!("Couldn't get this torrent's file list: {e:#}"));
                    r.task = None;
                }
                _ => {}
            },
        }
    });
    resolving.insert(id, Resolving { since: std::time::Instant::now(), error: None, task: Some(task) });
}

#[tauri::command]
fn retry_download(app: AppHandle, state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    let item = db::get(&state.db.lock().unwrap(), id).map_err(err)?;
    let magnet = item.torrent_url.filter(|u| u.starts_with("magnet:")).ok_or("this download has no magnet link to retry")?;
    state.resolving.lock().unwrap().remove(&id);
    spawn_resolve(&app, id, magnet, item.single_video);
    Ok(())
}

fn resume_resolving(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(items) = db::list(&state.db.lock().unwrap()) else { return };
    for item in items {
        if let Some(magnet) = item.torrent_url.filter(|u| u.starts_with("magnet:")) {
            if find_torrent(&state.session, &item.info_hash).is_none() {
                spawn_resolve(app, item.id, magnet, item.single_video);
            }
        }
    }
}

///retries matching for downloads that couldn't be identified when they were added.
async fn identify_unmatched(app: AppHandle) {
    let state = app.state::<AppState>();
    let Ok(items) = db::list(&state.db.lock().unwrap()) else { return };
    for item in items.into_iter().filter(|i| i.tmdb_id.is_none()) {
        let mut parsed = release::parse(&item.title);
        parsed.year = parsed.year.or(item.year);
        parsed.season = parsed.season.or(item.season);
        parsed.episode = parsed.episode.or(item.episode);
        identify(&state, item.id, parsed).await;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryEntry {
    #[serde(flatten)]
    item: db::Item,
    /// resolving | downloading | paused | done | starting | error | missing
    status: &'static str,
    progress: f64,
    downloaded_bytes: u64,
    total_bytes: u64,
    speed_mibps: f64,
    eta: Option<String>,
    error: Option<String>,
    resolving_secs: Option<u64>,
    videos: Vec<VideoEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VideoEntry {
    #[serde(flatten)]
    video: db::Video,
    progress: f64,
    playable: bool,
    ready_in_secs: Option<u64>,
    resume_at: Option<f64>,
}

fn video_entry(video: db::Video, file_progress: &[u64], speed_mibps: f64) -> VideoEntry {
    let have = file_progress.get(video.file_index as usize).copied().unwrap_or(0);
    let missing = player::buffer_target(video.len).saturating_sub(have);
    let ready_in_secs =
        (missing > 0 && speed_mibps > 0.0).then(|| (missing as f64 / (speed_mibps * 1024.0 * 1024.0)).ceil() as u64);
    VideoEntry {
        progress: if video.len > 0 { have as f64 / video.len as f64 } else { 0.0 },
        playable: video.len > 0 && (have >= video.len || missing == 0),
        ready_in_secs,
        resume_at: player::resume_from(video.position_secs, video.duration_secs),
        video,
    }
}

fn local_entry(state: &AppState, item: db::Item) -> LibraryEntry {
    let videos = db::videos_for(&state.db.lock().unwrap(), item.id).unwrap_or_default();
    let progress = Files::of(state, &item).map(|f| f.progress(&videos)).unwrap_or_default();
    let on_disk: u64 = progress.iter().sum();
    LibraryEntry {
        status: if on_disk > 0 { "done" } else { "missing" },
        progress: 1.0,
        downloaded_bytes: on_disk,
        total_bytes: videos.iter().map(|v| v.len).sum(),
        speed_mibps: 0.0,
        eta: None,
        error: (on_disk == 0).then(|| "The file is no longer in the folder.".into()),
        resolving_secs: None,
        videos: videos.into_iter().map(|v| video_entry(v, &progress, 0.0)).collect(),
        item,
    }
}

fn find_torrent(session: &Session, info_hash: &str) -> Option<Arc<ManagedTorrent>> {
    let hash = Id20::from_str(info_hash).ok()?;
    session.get(TorrentIdOrHash::Hash(hash))
}

#[tauri::command]
fn library(state: State<'_, AppState>) -> CmdResult<Vec<LibraryEntry>> {
    let items = db::list(&state.db.lock().unwrap()).map_err(err)?;
    Ok(items
        .into_iter()
        .map(|mut item| {
            if item.local_path.is_some() {
                return local_entry(&state, item);
            }
            let Some(handle) = find_torrent(&state.session, &item.info_hash) else {
                let videos = db::videos_for(&state.db.lock().unwrap(), item.id).unwrap_or_default();
                let (status, error, resolving_secs) = match state.resolving.lock().unwrap().get(&item.id) {
                    Some(r) => match &r.error {
                        Some(e) => ("error", Some(e.clone()), None),
                        None => ("resolving", None, Some(r.since.elapsed().as_secs())),
                    },
                    None => ("missing", Some("The torrent is no longer in the download engine.".into()), None),
                };
                return LibraryEntry {
                    item,
                    status,
                    progress: 0.0,
                    downloaded_bytes: 0,
                    total_bytes: 0,
                    speed_mibps: 0.0,
                    eta: None,
                    error,
                    resolving_secs,
                    videos: videos.into_iter().map(|v| video_entry(v, &[], 0.0)).collect(),
                };
            };
            if item.title.is_empty() {
                item.title = handle.name().unwrap_or_else(|| "Untitled download".into());
            }
            let stats = handle.stats();
            let status = match stats.state {
                TorrentStatsState::Error => "error",
                _ if stats.finished => "done",
                TorrentStatsState::Paused => "paused",
                TorrentStatsState::Initializing { .. } => "starting",
                TorrentStatsState::Live => "downloading",
            };
            let progress = if stats.total_bytes > 0 {
                stats.progress_bytes as f64 / stats.total_bytes as f64
            } else {
                0.0
            };
            let speed = stats.live.as_ref().map_or(0.0, |l| l.download_speed.mbps);
            let videos = ensure_videos(&state, &item, &Files::Torrent(handle.clone())).unwrap_or_default();
            LibraryEntry {
                status,
                progress,
                downloaded_bytes: stats.progress_bytes,
                total_bytes: stats.total_bytes,
                speed_mibps: speed,
                eta: stats.live.as_ref().and_then(|l| l.time_remaining.as_ref()).map(|t| t.to_string()),
                resolving_secs: None,
                videos: videos.into_iter().map(|v| video_entry(v, &stats.file_progress, speed)).collect(),
                error: stats.error,
                item,
            }
        })
        .collect())
}

fn torrent_for_item(state: &AppState, id: i64) -> anyhow::Result<(db::Item, Option<Arc<ManagedTorrent>>)> {
    let item = db::get(&state.db.lock().unwrap(), id)?;
    let handle = find_torrent(&state.session, &item.info_hash);
    Ok((item, handle))
}

#[tauri::command]
async fn pause(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    let (_, handle) = torrent_for_item(&state, id).map_err(err)?;
    let handle = handle.ok_or("torrent not found")?;
    state.session.pause(&handle).await.map_err(err)
}

#[tauri::command]
async fn resume(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    let (_, handle) = torrent_for_item(&state, id).map_err(err)?;
    let handle = handle.ok_or("torrent not found")?;
    state.session.unpause(&handle).await.map_err(err)
}

fn move_to_trash(path: &Path) -> CmdResult<()> {
    if !path.exists() {
        return Ok(());
    }
    trash::delete(path).map_err(|e| format!("Couldn't move {} to the Trash: {e}", path.display()))
}

#[tauri::command]
async fn remove(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    if let Some(resolving) = state.resolving.lock().unwrap().remove(&id) {
        if let Some(task) = resolving.task {
            task.abort();
        }
    }
    let (item, handle) = torrent_for_item(&state, id).map_err(err)?;
    if let Some(path) = &item.local_path {
        let subs = db::list_subtitles(&state.db.lock().unwrap(), id, 0).map_err(err)?;
        for sub in subs.iter().filter(|s| s.source != "torrent") {
            move_to_trash(Path::new(&sub.path))?;
        }
        move_to_trash(Path::new(path))?;
    }
    if let Some(handle) = handle {
        let folder = handle.output_folder().to_path_buf();
        // Forget the torrent first so nothing still writes to its files.
        state
            .session
            .delete(TorrentIdOrHash::Hash(handle.info_hash()), false)
            .await
            .map_err(err)?;
        if folder != state.download_dir && folder.starts_with(&state.download_dir) {
            move_to_trash(&folder)?;
        } else {
            let videos = db::videos_for(&state.db.lock().unwrap(), id).map_err(err)?;
            for video in &videos {
                move_to_trash(&folder.join(&video.path))?;
            }
        }
    }
    db::delete(&state.db.lock().unwrap(), id).map_err(err)
}

#[tauri::command]
async fn play(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    file_index: Option<i64>,
    from_start: Option<bool>,
) -> Result<(), PlayError> {
    let (item, files, video) = resolve(&state, id, file_index).map_err(|_| PlayError::FileMissing)?;
    let show_hint = player::take_hint(&state.db.lock().unwrap())?;
    let launch = prepare_launch(&state, item, &files, video, from_start.unwrap_or(false), show_hint).await?;
    state.player.start(&app, launch).await
}

async fn prepare_launch(
    state: &AppState,
    item: db::Item,
    files: &Files,
    video: db::Video,
    from_start: bool,
    show_hint: bool,
) -> Result<player::Launch, PlayError> {
    let index = video.file_index as usize;
    let have = files.progress(std::slice::from_ref(&video)).get(index).copied().unwrap_or(0);
    let video_path = files.folder().join(&video.path);
    let source = if have >= video.len {
        if !video_path.exists() {
            return Err(PlayError::FileMissing);
        }
        video_path.to_string_lossy().into_owned()
    } else {
        let Files::Torrent(handle) = files else { return Err(PlayError::FileMissing) };
        if have < player::buffer_target(video.len) {
            return Err(PlayError::NotReady);
        }
        if matches!(handle.stats().state, TorrentStatsState::Paused) {
            state.session.unpause(handle).await?;
        }
        state.stream.url(&item.info_hash, index)
    };

    if let Files::Torrent(handle) = files {
        let _ = sync_release_subtitles(state, &item, handle);
    }
    let (tracks, languages, subtitle_files) = {
        let db = state.db.lock().unwrap();
        let show = item.tmdb_id.filter(|_| item.kind.as_deref() == Some("tv"));
        let tracks = match db::get_tracks(&db, item.id, video.file_index)? {
            Some(t) => t,
            None => show.map(|s| db::get_show_tracks(&db, s)).transpose()?.flatten().unwrap_or_default(),
        };
        let languages = subtitles::load_settings(&db)?.languages;
        // mpv auto-loads subtitles beside the video; pass the rest explicitly.
        let subtitle_files = db::list_subtitles(&db, item.id, video.file_index)?
            .into_iter()
            .map(|s| PathBuf::from(s.path))
            .filter(|p| !subtitles::loads_by_itself(p, &video_path) && p.exists())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        (tracks, languages, subtitle_files)
    };
    let start = if from_start { None } else { player::resume_from(video.position_secs, video.duration_secs) };
    Ok(player::Launch { item, video, source, start, tracks, languages, show_hint, subtitle_files })
}

async fn next_episode_launch(state: &AppState, item_id: i64, file_index: i64) -> Option<player::Launch> {
    let current = db::get(&state.db.lock().unwrap(), item_id).ok()?;
    let show = current.tmdb_id.filter(|_| current.kind.as_deref() == Some("tv"))?;
    let items: Vec<_> = db::list(&state.db.lock().unwrap()).ok()?.into_iter().filter(|i| i.tmdb_id == Some(show)).collect();
    let mut episodes = vec![];
    for item in items {
        let Some(files) = Files::of(state, &item) else { continue };
        let videos = ensure_videos(state, &item, &files).unwrap_or_default();
        let progress = files.progress(&videos);
        for video in videos {
            let have = progress.get(video.file_index as usize).copied().unwrap_or(0);
            if let (Some(season), Some(episode)) = (video.season, video.episode) {
                episodes.push(((season, episode), have >= player::buffer_target(video.len), item.clone(), files.clone(), video));
            }
        }
    }
    let now = episodes.iter().find(|e| e.2.id == item_id && e.4.file_index == file_index)?.0;
    let (_, _, item, files, video) = episodes
        .into_iter()
        .filter(|(key, playable, ..)| follows(now, *key) && *playable)
        .min_by_key(|(key, ..)| *key)?;
    prepare_launch(state, item, &files, video, false, false).await.ok()
}

fn follows(now: (u32, u32), next: (u32, u32)) -> bool {
    next == (now.0, now.1 + 1) || next == (now.0 + 1, 1)
}

#[tauri::command]
fn player_state(state: State<'_, AppState>) -> Option<player::PlayerState> {
    state.player.state()
}

#[tauri::command]
fn player_command(state: State<'_, AppState>, args: Vec<serde_json::Value>) -> CmdResult<()> {
    state.player.command(args).map_err(err)
}

#[tauri::command]
fn stop_player(app: AppHandle, state: State<'_, AppState>) {
    state.player.stop(&app);
}

#[tauri::command]
fn mark_watched(state: State<'_, AppState>, id: i64, file_index: i64, watched: bool) -> CmdResult<()> {
    db::set_watched(&state.db.lock().unwrap(), id, file_index, watched).map_err(err)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackSettings {
    autoplay_next: bool,
}

#[tauri::command]
fn playback_settings(state: State<'_, AppState>) -> CmdResult<PlaybackSettings> {
    let value = db::get_setting(&state.db.lock().unwrap(), "autoplay_next").map_err(err)?;
    Ok(PlaybackSettings { autoplay_next: value.as_deref() != Some("0") })
}

#[tauri::command]
fn save_playback_settings(state: State<'_, AppState>, settings: PlaybackSettings) -> CmdResult<()> {
    let value = if settings.autoplay_next { "1" } else { "0" };
    db::set_setting(&state.db.lock().unwrap(), "autoplay_next", value).map_err(err)
}

#[tauri::command]
fn mark_season_watched(state: State<'_, AppState>, tmdb_id: i64, season: u32, watched: bool) -> CmdResult<()> {
    db::set_season_watched(&state.db.lock().unwrap(), tmdb_id, season, watched).map_err(err)
}

fn sync_release_subtitles(state: &AppState, item: &db::Item, handle: &Arc<ManagedTorrent>) -> anyhow::Result<()> {
    let videos = ensure_videos(state, item, &Files::Torrent(handle.clone()))?;
    let progress = handle.stats().file_progress;
    let files: Vec<PathBuf> = torrent_files(handle)?
        .into_iter()
        .filter(|(i, p, len)| {
            subtitles::is_subtitle(p) && *len <= subtitles::MAX_SUBTITLE_BYTES && progress.get(*i) == Some(len)
        })
        .map(|(_, p, _)| p)
        .collect();
    let db = state.db.lock().unwrap();
    for relative in files {
        let name = relative.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let parsed = release::parse(&name);
        let video = match parsed.episode {
            Some(episode) => videos.iter().find(|v| v.episode == Some(episode)),
            None if videos.len() == 1 => videos.first(),
            None => None,
        };
        let path = handle.output_folder().join(&relative);
        let Some(video) = video.filter(|_| path.exists()) else { continue };
        db::insert_subtitle(
            &db,
            &db::NewSubtitle {
                item_id: item.id,
                file_index: video.file_index,
                lang: subtitles::guess_language(&relative),
                label: &name,
                path: &path.to_string_lossy(),
                source: "torrent",
                hearing_impaired: name.to_lowercase().contains("sdh"),
                provider_key: None,
            },
        )?;
    }
    Ok(())
}

#[tauri::command]
fn subtitle_settings(state: State<'_, AppState>) -> CmdResult<subtitles::Settings> {
    subtitles::load_settings(&state.db.lock().unwrap()).map_err(err)
}

#[tauri::command]
async fn save_subtitle_settings(state: State<'_, AppState>, mut settings: subtitles::Settings) -> CmdResult<()> {
    if settings.opensubtitles_password.is_empty() && !settings.opensubtitles_user.trim().is_empty() {
        settings.opensubtitles_password =
            subtitles::load_settings(&state.db.lock().unwrap()).map_err(err)?.opensubtitles_password;
    }
    state.subtitles.sign_out().await;
    if settings.has_login() {
        if settings.opensubtitles_key.trim().is_empty() {
            return Err("Add your OpenSubtitles API key to sign in.".into());
        }
        state.subtitles.sign_in(&state.http, &settings).await.map_err(err)?;
    }
    subtitles::save_settings(&state.db.lock().unwrap(), &settings).map_err(err)
}

#[tauri::command]
fn list_subtitles(state: State<'_, AppState>, id: i64, file_index: Option<i64>) -> CmdResult<Vec<db::Subtitle>> {
    let (item, files, video) = resolve(&state, id, file_index).map_err(err)?;
    if let Files::Torrent(handle) = &files {
        sync_release_subtitles(&state, &item, handle).map_err(err)?;
    }
    db::list_subtitles(&state.db.lock().unwrap(), id, video.file_index).map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubtitleSearch {
    results: Vec<subtitles::Ranked>,
    errors: Vec<String>,
    quota: Option<subtitles::Quota>,
}

#[tauri::command]
async fn search_subtitles(
    state: State<'_, AppState>,
    id: i64,
    file_index: Option<i64>,
    languages: Option<Vec<String>>,
) -> CmdResult<SubtitleSearch> {
    let settings = subtitles::load_settings(&state.db.lock().unwrap()).map_err(err)?;
    if !settings.any_provider() {
        return Err("Add an OpenSubtitles or Subdl API key in Sources to search for subtitles.".into());
    }
    let query =
        subtitle_query(&state, id, file_index, languages, &settings, Duration::from_secs(8)).await.map_err(err)?;
    let (results, errors) = state.subtitles.search(&state.http, &settings, &query).await;
    Ok(SubtitleSearch { results, errors, quota: state.subtitles.quota() })
}

async fn subtitle_query(
    state: &AppState,
    id: i64,
    file_index: Option<i64>,
    languages: Option<Vec<String>>,
    settings: &subtitles::Settings,
    hash_wait: Duration,
) -> anyhow::Result<subtitles::Query> {
    let (item, files, video) = resolve(state, id, file_index)?;
    let mut languages = languages.filter(|l| !l.is_empty()).unwrap_or_else(|| settings.languages.clone());
    if languages.is_empty() {
        languages = vec!["en".into()];
    }
    Ok(subtitles::Query {
        tmdb_id: item.tmdb_id,
        tv: item.kind.as_deref() == Some("tv") || video.season.is_some(),
        title: item.title,
        year: item.year,
        season: video.season,
        episode: video.episode,
        languages,
        release_name: Path::new(&video.path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        moviehash: match &files {
            Files::Torrent(handle) => {
                subtitles::moviehash_of(handle.clone(), video.file_index as usize, video.len, hash_wait).await
            }
            Files::Local(dir) => subtitles::moviehash_of_file(&dir.join(&video.path)),
        },
    })
}

#[tauri::command]
async fn download_subtitle(
    state: State<'_, AppState>,
    id: i64,
    file_index: Option<i64>,
    found: subtitles::Found,
) -> CmdResult<db::Subtitle> {
    fetch_subtitle(&state, id, file_index, &found).await.map_err(err)
}

async fn fetch_subtitle(
    state: &AppState,
    id: i64,
    file_index: Option<i64>,
    found: &subtitles::Found,
) -> anyhow::Result<db::Subtitle> {
    let settings = subtitles::load_settings(&state.db.lock().unwrap())?;
    let (_, files, video) = resolve(state, id, file_index)?;
    let (text, ext) = state.subtitles.download(&state.http, &settings, found, video.season, video.episode).await?;
    let path = subtitles::save_beside(&files.folder().join(&video.path), &found.lang, &ext, &text)?;
    let path = path.to_string_lossy();
    let subtitle = db::insert_subtitle(
        &state.db.lock().unwrap(),
        &db::NewSubtitle {
            item_id: id,
            file_index: video.file_index,
            lang: Some(&found.lang),
            label: &found.release,
            path: &path,
            source: &found.provider,
            hearing_impaired: found.hearing_impaired,
            provider_key: Some(&found.key),
        },
    )?;
    if state.player.state().is_some_and(|p| p.item_id == id && p.file_index == video.file_index) {
        let args = ["sub-add", &path, "select", &found.release, &found.lang];
        let _ = state.player.command(args.iter().map(|a| serde_json::Value::from(*a)).collect());
    }
    Ok(subtitle)
}

async fn auto_fetch_subtitles(app: AppHandle) {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let state = app.state::<AppState>();
        if let Err(e) = auto_fetch_once(&state).await {
            eprintln!("subtitles: automatic download failed: {e:#}");
        }
    }
}

async fn auto_fetch_once(state: &AppState) -> anyhow::Result<()> {
    let (settings, items) = {
        let db = state.db.lock().unwrap();
        (subtitles::load_settings(&db)?, db::list(&db)?)
    };
    let Some(lang) = settings.languages.first().cloned() else { return Ok(()) };
    if !settings.auto_fetch || !settings.any_provider() {
        return Ok(());
    }
    for item in items.into_iter().filter(|i| i.tmdb_id.is_some()) {
        let Some(files) = Files::of(state, &item) else { continue };
        let videos = ensure_videos(state, &item, &files).unwrap_or_default();
        let progress = files.progress(&videos);
        for video in videos {
            let key = (item.id, video.file_index);
            let have = progress.get(video.file_index as usize).copied().unwrap_or(0);
            if video.watched || have < player::buffer_target(video.len) || state.subtitles.attempted.lock().unwrap().contains(&key) {
                continue;
            }
            state.subtitles.attempted.lock().unwrap().insert(key);
            if let Some(handle) = files.torrent() {
                sync_release_subtitles(state, &item, handle)?;
            }
            let existing = db::list_subtitles(&state.db.lock().unwrap(), item.id, video.file_index)?;
            if existing.iter().any(|s| s.lang.as_deref().is_some_and(|l| l.starts_with(&lang))) {
                continue;
            }
            let wait = Duration::from_secs(30);
            let query = subtitle_query(state, item.id, Some(video.file_index), Some(vec![lang]), &settings, wait).await?;
            let (results, _) = state.subtitles.search(&state.http, &settings, &query).await;
            if let Some(best) = results.first() {
                fetch_subtitle(state, item.id, Some(video.file_index), &best.found).await?;
            }
            // Once a minute stays within provider rate limits.
            return Ok(());
        }
    }
    Ok(())
}

async fn watch_folder(app: AppHandle) {
    loop {
        tokio::time::sleep(Duration::from_secs(20)).await;
        let state = app.state::<AppState>();
        if let Err(e) = scan_folder(&state).await {
            eprintln!("library folder: {e:#}");
        }
    }
}

#[tauri::command]
async fn scan_library_folder(state: State<'_, AppState>) -> CmdResult<usize> {
    // A file whose size holds steady has finished copying.
    let first = scan_folder(&state).await.map_err(err)?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    Ok(first + scan_folder(&state).await.map_err(err)?)
}

#[tauri::command]
fn open_library_folder(app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
    app.opener().open_path(state.download_dir.to_string_lossy(), None::<&str>).map_err(err)?;
    Ok(state.download_dir.to_string_lossy().into_owned())
}

async fn scan_folder(state: &AppState) -> anyhow::Result<usize> {
    let owned: HashSet<PathBuf> = state.session.with_torrents(|torrents| {
        torrents
            .flat_map(|(_, handle)| {
                let folder = handle.output_folder().to_path_buf();
                handle
                    .with_metadata(|m| m.file_infos.iter().map(|f| folder.join(&f.relative_filename)).collect::<Vec<_>>())
                    .unwrap_or_default()
            })
            .collect()
    });
    let items = db::list(&state.db.lock().unwrap())?;
    let torrent_videos: Vec<String> = {
        let db = state.db.lock().unwrap();
        items
            .iter()
            .filter(|i| i.local_path.is_none())
            .flat_map(|i| db::videos_for(&db, i.id).unwrap_or_default())
            .map(|v| v.path)
            .collect()
    };
    let known: HashMap<String, i64> = items.iter().filter_map(|i| Some((i.local_path.clone()?, i.id))).collect();
    for (path, id) in &known {
        if !Path::new(path).exists() {
            db::delete(&state.db.lock().unwrap(), *id)?;
        }
    }

    let found = local::find_videos(&state.download_dir, is_video);
    let mut added = vec![];
    {
        let mut sizes = state.folder_sizes.lock().unwrap();
        let before = std::mem::take(&mut *sizes);
        let db = state.db.lock().unwrap();
        for (path, len) in found {
            let key = path.to_string_lossy().into_owned();
            let in_torrent = owned.contains(&path) || torrent_videos.iter().any(|v| path.ends_with(v));
            if in_torrent || known.contains_key(&key) || len < local::MIN_VIDEO_BYTES || is_sample(&path, len, u64::MAX) {
                continue;
            }
            sizes.insert(path.clone(), len);
            if before.get(&path) != Some(&len) {
                continue;
            }
            let parsed = local::describe(&path);
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let title = if parsed.title.is_empty() { name.clone() } else { parsed.title.clone() };
            let id = db::insert(
                &db,
                &db::NewItem {
                    title: &title,
                    year: parsed.year,
                    source: "Your files",
                    info_hash: &format!("file:{key}"),
                    poster: None,
                    tmdb_id: None,
                    kind: None,
                    season: parsed.season,
                    episode: parsed.episode,
                    torrent_url: None,
                    single_video: false,
                    local_path: Some(&key),
                },
            )?;
            let video = db::NewVideo { file_index: 0, path: name, len, season: parsed.season, episode: parsed.episode };
            db::save_videos(&db, id, &[video], 0)?;
            added.push((id, parsed));
        }
    }
    let count = added.len();
    for (id, parsed) in added {
        identify(state, id, parsed).await;
    }
    Ok(count)
}

#[tauri::command]
fn open_external(app: AppHandle, state: State<'_, AppState>, id: i64, file_index: Option<i64>) -> CmdResult<()> {
    let (_, files, video) = resolve(&state, id, file_index).map_err(err)?;
    let path = files.folder().join(video.path);
    if !path.exists() {
        return Err("the video file hasn't been written yet".into());
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(err)
}

#[tauri::command]
fn reveal(app: AppHandle, state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    let item = db::get(&state.db.lock().unwrap(), id).map_err(err)?;
    let target = match (&item.local_path, Files::of(&state, &item)) {
        (Some(path), _) => PathBuf::from(path),
        (None, Some(files)) => files.folder(),
        (None, None) => state.download_dir.clone(),
    };
    app.opener().reveal_item_in_dir(target).map_err(err)
}

fn init_state(app: &AppHandle) -> anyhow::Result<AppState> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let download_dir = dirs::video_dir()
        .or_else(dirs::home_dir)
        .context("no home directory")?
        .join("cardamoi");
    std::fs::create_dir_all(&download_dir)?;

    let (session, stream) = tauri::async_runtime::block_on(async {
        let session = Session::new_with_opts(
            download_dir.clone(),
            SessionOptions {
                fastresume: true,
                disable_local_service_discovery: true,
                persistence: Some(SessionPersistenceConfig::Json { folder: Some(data_dir.join("session")) }),
                ..Default::default()
            },
        )
        .await?;
        let stream = stream::start(session.clone()).await?;
        anyhow::Ok((session, stream))
    })?;

    let db_conn = db::open(&data_dir.join("library.db"))?;
    let fs_config = db::get_flaresolverr_config(&db_conn)?;
    let http = Arc::new(FlareSolverrClient::new(fs_config)?);

    let prowlarr = Prowlarr::start(&app.path().resource_dir()?, &data_dir, http.clone());

    Ok(AppState {
        session,
        db: Mutex::new(db_conn),
        http,
        download_dir,
        prowlarr,
        stream,
        player: Player::default(),
        subtitles: subtitles::Subtitles::default(),
        resolving: Mutex::new(HashMap::new()),
        folder_sizes: Mutex::new(HashMap::new()),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let item = |id: &str, text: &str, accelerator: Option<&str>| {
        let builder = MenuItemBuilder::with_id(id, text);
        match accelerator {
            Some(keys) => builder.accelerator(keys).build(app),
            None => builder.build(app),
        }
    };

    let app_menu = SubmenuBuilder::new(app, "cardamoi")
        .about(None)
        .separator()
        .item(&item("preferences", "Preferences…", Some("CmdOrCtrl+Comma"))?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;
    let file = SubmenuBuilder::new(app, "File")
        .item(&item("add-magnet", "Add Magnet Link…", Some("CmdOrCtrl+U"))?)
        .separator()
        .item(&item("get-info", "Get Info", Some("CmdOrCtrl+I"))?)
        .item(&item("reveal", "Show in Finder", Some("CmdOrCtrl+Shift+R"))?)
        .item(&item("open-external", "Open in System Player", None)?)
        .separator()
        .item(&item("open-folder", "Open the Library Folder", None)?)
        .item(&item("check-folder", "Check Folder for New Videos", None)?)
        .separator()
        .close_window()
        .build()?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .separator()
        .item(&item("find", "Find", Some("CmdOrCtrl+F"))?)
        .build()?;
    let view = SubmenuBuilder::new(app, "View")
        .item(&item("view-grid", "as Grid", Some("CmdOrCtrl+Digit1"))?)
        .item(&item("view-list", "as List", Some("CmdOrCtrl+Digit2"))?)
        .separator()
        .item(&item("go-upnext", "Up Next", Some("CmdOrCtrl+Alt+Digit1"))?)
        .item(&item("go-films", "Films", Some("CmdOrCtrl+Alt+Digit2"))?)
        .item(&item("go-shows", "TV Shows", Some("CmdOrCtrl+Alt+Digit3"))?)
        .item(&item("go-downloads", "Downloads", Some("CmdOrCtrl+Alt+Digit4"))?)
        .separator()
        .item(&item("back", "Back", Some("CmdOrCtrl+BracketLeft"))?)
        .item(&item("forward", "Forward", Some("CmdOrCtrl+BracketRight"))?)
        .separator()
        .fullscreen()
        .build()?;
    // No Play shortcut: Space must still type in text fields.
    let controls = SubmenuBuilder::new(app, "Controls")
        .item(&item("play", "Play", None)?)
        .item(&item("play-start", "Play from Beginning", None)?)
        .item(&item("stop", "Stop", Some("CmdOrCtrl+Period"))?)
        .separator()
        .item(&item("subtitles", "Subtitles…", Some("CmdOrCtrl+T"))?)
        .item(&item("toggle-watched", "Mark as Watched or Unwatched", Some("CmdOrCtrl+K"))?)
        .build()?;
    let window = SubmenuBuilder::new(app, "Window").minimize().maximize().build()?;
    let help = SubmenuBuilder::new(app, "Help")
        .item(&item("help", "cardamoi Help", Some("CmdOrCtrl+Shift+Slash"))?)
        .build()?;
    Menu::with_items(app, &[&app_menu, &file, &edit, &view, &controls, &window, &help])
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = init_state(app.handle())?;
            app.manage(state);
            tauri::async_runtime::spawn(auto_fetch_subtitles(app.handle().clone()));
            resume_resolving(app.handle());
            tauri::async_runtime::spawn(identify_unmatched(app.handle().clone()));
            tauri::async_runtime::spawn(watch_folder(app.handle().clone()));
            app.set_menu(build_menu(app.handle())?)?;
            app.on_menu_event(|app, event| {
                let _ = app.emit("menu", event.id().0.as_str());
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
            catalog_search,
            title_details,
            season_episodes,
            prowlarr_status,
            open_prowlarr,
            list_sources,
            add_source,
            set_source_enabled,
            remove_source,
            get_flaresolverr_config,
            set_flaresolverr_config,
            test_flaresolverr,
            add_download,
            retry_download,
            scan_library_folder,
            open_library_folder,
            library,
            pause,
            resume,
            remove,
            play,
            player_state,
            player_command,
            stop_player,
            mark_watched,
            mark_season_watched,
            playback_settings,
            save_playback_settings,
            open_external,
            subtitle_settings,
            save_subtitle_settings,
            list_subtitles,
            search_subtitles,
            download_subtitle,
            reveal
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                if let Some(state) = app.try_state::<AppState>() {
                    state.player.stop(app);
                    state.prowlarr.stop();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(files: &[(&str, u64)]) -> Option<usize> {
        best_video(files.iter().enumerate().map(|(i, (p, len))| (i, Path::new(*p), *len)))
    }

    #[test]
    fn prefers_largest_mp4() {
        let files = [("film.avi", 900), ("film_512kb.mp4", 100), ("film.mp4", 400), ("film.gif", 5)];
        assert_eq!(pick(&files), Some(2));
    }

    #[test]
    fn falls_back_to_largest_video() {
        let files = [("film.ogv", 300), ("film.MKV", 700), ("cover.jpg", 10)];
        assert_eq!(pick(&files), Some(1));
    }

    #[test]
    fn none_without_video() {
        assert_eq!(pick(&[("notes.txt", 1), ("cover.jpg", 2)]), None);
    }

    fn item(kind: &str, season: Option<u32>, episode: Option<u32>) -> db::Item {
        db::Item {
            id: 1,
            title: "Severance".into(),
            year: None,
            source: "x".into(),
            info_hash: "aa".into(),
            poster: None,
            added_at: 0,
            tmdb_id: Some(95396),
            kind: Some(kind.into()),
            season,
            episode,
            torrent_url: None,
            single_video: false,
            local_path: None,
        }
    }

    fn files(list: &[(&str, u64)]) -> Vec<(usize, PathBuf, u64)> {
        list.iter().enumerate().map(|(i, (p, len))| (i, PathBuf::from(p), *len)).collect()
    }

    #[test]
    fn season_packs_list_every_episode() {
        let pack = files(&[
            ("Severance.S01/Severance.S01E02.1080p.mkv", 900),
            ("Severance.S01/Severance.S01E01.1080p.mkv", 1000),
            ("Severance.S01/Sample/severance.s01e01.sample.mkv", 20),
            ("Severance.S01/Severance.S01E01.1080p.en.srt", 1),
        ]);
        let (videos, main) = discover_videos(&item("tv", Some(1), None), &pack).unwrap();
        let episodes: Vec<_> = videos.iter().map(|v| (v.file_index, v.season, v.episode)).collect();
        assert_eq!(episodes, [(1, Some(1), Some(1)), (0, Some(1), Some(2))]);
        assert_eq!(main, 1);
    }

    #[test]
    fn a_single_episode_or_film_is_one_video() {
        let one = files(&[("Show.S02E03.mkv", 900), ("Show.S02E03.sample.mkv", 10), ("info.nfo", 1)]);
        let (videos, main) = discover_videos(&item("tv", Some(2), Some(3)), &one).unwrap();
        assert_eq!((videos.len(), main, videos[0].season, videos[0].episode), (1, 0, Some(2), Some(3)));

        let film = files(&[("Dune.2021.mkv", 9000), ("Extras/Part.E01.mkv", 100), ("Extras/Part.E02.mkv", 100)]);
        let (videos, main) = discover_videos(&item("movie", None, None), &film).unwrap();
        assert_eq!((videos.len(), main, videos[0].episode), (1, 0, None));

        assert!(discover_videos(&item("movie", None, None), &files(&[("readme.txt", 1)])).is_none());
    }

    fn video(file_index: i64, position_secs: f64, watched: bool, last_played_at: Option<i64>) -> db::Video {
        db::Video { file_index, position_secs, duration_secs: Some(3000.0), watched, last_played_at, ..Default::default() }
    }

    #[test]
    fn episodes_that_follow() {
        assert!(follows((2, 4), (2, 5)));
        assert!(follows((2, 10), (3, 1)));
        assert!(!follows((2, 4), (2, 6)));
        assert!(!follows((2, 4), (3, 2)));
        assert!(!follows((2, 4), (2, 4)));
    }

    #[test]
    fn next_up_prefers_where_you_left_off() {
        let pick = |videos| next_up(videos).map(|v| v.file_index);
        assert_eq!(pick(vec![video(0, 600.0, false, Some(1)), video(1, 900.0, false, Some(5))]), Some(1));
        assert_eq!(pick(vec![video(0, 0.0, true, Some(1)), video(1, 0.0, false, None), video(2, 0.0, false, None)]), Some(1));
        assert_eq!(pick(vec![video(0, 0.0, true, None), video(1, 0.0, true, None)]), Some(0));
        assert_eq!(pick(vec![]), None);
    }
}
