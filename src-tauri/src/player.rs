use std::{
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    process::Child,
    sync::{mpsc, oneshot},
};

use crate::{AppState, db};

const MIN_RESUME_SECS: f64 = 30.0;
const WATCHED_AT: f64 = 0.9;
const RESUME_REWIND_SECS: f64 = 5.0;
const SAVE_EVERY: Duration = Duration::from_secs(5);
const EMIT_EVERY: Duration = Duration::from_millis(250);
const HINT_PLAYS: u32 = 3;
const TRACK_LIST_REQUEST: u64 = 100;
const AUTOPLAY_SECS: u32 = 10;

const INPUT_CONF: &str = include_str!("mpv-input.conf");

pub fn resume_from(position: f64, duration: Option<f64>) -> Option<f64> {
    let before_end = duration.is_none_or(|d| position < d * WATCHED_AT);
    (position > MIN_RESUME_SECS && before_end).then(|| (position - RESUME_REWIND_SECS).max(0.0))
}

pub fn is_watched(position: f64, duration: Option<f64>) -> bool {
    duration.is_some_and(|d| d > 0.0 && position >= d * WATCHED_AT)
}

pub fn buffer_target(len: u64) -> u64 {
    (len / 20).min(50 * 1024 * 1024)
}

pub fn take_hint(conn: &rusqlite::Connection) -> rusqlite::Result<bool> {
    let shown: u32 = db::get_setting(conn, "hint_plays")?.and_then(|v| v.parse().ok()).unwrap_or(0);
    if shown >= HINT_PLAYS {
        return Ok(false);
    }
    db::set_setting(conn, "hint_plays", &(shown + 1).to_string())?;
    Ok(true)
}

pub fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlayError {
    MpvMissing,
    FileMissing,
    NotReady,
    Other { message: String },
}

impl<E: std::fmt::Display> From<E> for PlayError {
    fn from(e: E) -> Self {
        PlayError::Other { message: format!("{e:#}") }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "kebab-case"))]
pub struct Track {
    pub id: i64,
    #[serde(rename(deserialize = "type"))]
    pub kind: String,
    pub lang: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub external: bool,
    #[serde(default)]
    pub selected: bool,
    #[serde(default)]
    pub forced: bool,
    #[serde(default)]
    pub hearing_impaired: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub item_id: i64,
    pub file_index: i64,
    pub title: String,
    /// opening | playing | ended | stopped | failed | crashed
    pub status: &'static str,
    pub position: f64,
    pub duration: Option<f64>,
    pub paused: bool,
    pub buffering: bool,
    pub watched: bool,
    pub tracks: Vec<Track>,
    pub sub_delay: f64,
    pub error: Option<String>,
}

pub struct Launch {
    pub item: db::Item,
    pub video: db::Video,
    pub source: String,
    pub start: Option<f64>,
    pub tracks: db::TrackPrefs,
    pub languages: Vec<String>,
    pub show_hint: bool,
    pub subtitle_files: Vec<String>,
}

#[derive(Default)]
pub struct Player {
    current: Mutex<Option<Live>>,
    generation: AtomicU64,
}

struct Live {
    generation: u64,
    commands: mpsc::UnboundedSender<Value>,
    state: Arc<Mutex<PlayerState>>,
    stop: Option<oneshot::Sender<()>>,
}

impl Player {
    pub fn state(&self) -> Option<PlayerState> {
        self.current.lock().unwrap().as_ref().map(|l| l.state.lock().unwrap().clone())
    }

    /// Allowlisted, since mpv commands can run programs.
    pub fn command(&self, args: Vec<Value>) -> anyhow::Result<()> {
        const COMMANDS: &[&str] = &["sub-add", "sub-remove", "sub-reload", "set", "cycle", "add", "seek", "show-text"];
        const PROPERTIES: &[&str] = &["sid", "aid", "sub-delay", "sub-visibility", "pause", "volume", "mute", "fullscreen"];
        let name = args.first().and_then(Value::as_str).unwrap_or_default();
        anyhow::ensure!(COMMANDS.contains(&name), "the player doesn't accept \"{name}\"");
        if matches!(name, "set" | "cycle" | "add") {
            let property = args.get(1).and_then(Value::as_str).unwrap_or_default();
            anyhow::ensure!(PROPERTIES.contains(&property), "the player doesn't let \"{property}\" be changed");
        }
        let current = self.current.lock().unwrap();
        let live = current.as_ref().ok_or_else(|| anyhow::anyhow!("nothing is playing"))?;
        live.commands.send(json!({ "command": args }))?;
        Ok(())
    }

    pub fn stop(&self, app: &AppHandle) {
        let Some(mut live) = self.current.lock().unwrap().take() else { return };
        let state = live.state.lock().unwrap().clone();
        with_db(app, |db| db::save_position(db, state.item_id, state.file_index, state.position, state.duration));
        if let Some(stop) = live.stop.take() {
            let _ = stop.send(());
        }
    }

    fn clear(&self, generation: u64) {
        let mut current = self.current.lock().unwrap();
        if current.as_ref().is_some_and(|l| l.generation == generation) {
            *current = None;
        }
    }

    pub async fn start(&self, app: &AppHandle, launch: Launch) -> Result<(), PlayError> {
        self.stop(app);
        let mpv = find_mpv(app).ok_or(PlayError::MpvMissing)?;
        let data_dir = app.path().app_data_dir()?;
        let input_conf = data_dir.join("mpv-input.conf");
        std::fs::write(&input_conf, INPUT_CONF)?;
        let socket = ipc_path(&data_dir);

        let label = launch_label(&launch);
        let mut child = tokio::process::Command::new(&mpv)
            .args(mpv_args(&launch, &label, &socket, &input_conf))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("couldn't start mpv ({}): {e}", mpv.display()))?;

        // mpv creates the socket shortly after starting.
        let deadline = Instant::now() + Duration::from_secs(10);
        let stream = loop {
            match connect(&socket).await {
                Ok(stream) => break stream,
                Err(_) if Instant::now() < deadline => {
                    if let Ok(Some(status)) = child.try_wait() {
                        return Err(format!("mpv closed before playing ({status})").into());
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    let _ = child.start_kill();
                    return Err(format!("couldn't connect to mpv: {e}").into());
                }
            }
        };

        let state = Arc::new(Mutex::new(PlayerState {
            item_id: launch.item.id,
            file_index: launch.video.file_index,
            title: label,
            status: "opening",
            position: launch.start.unwrap_or(0.0),
            duration: launch.video.duration_secs,
            watched: launch.video.watched,
            ..Default::default()
        }));
        let (commands, command_rx) = mpsc::unbounded_channel();
        let (stop, stop_rx) = oneshot::channel();
        let generation = self.generation.fetch_add(1, Ordering::Relaxed);
        *self.current.lock().unwrap() = Some(Live { generation, commands, state: state.clone(), stop: Some(stop) });

        let after_load = osd_message(launch.start, launch.show_hint)
            .map(|text| vec![json!({ "command": ["show-text", text, 4000] })])
            .unwrap_or_default();
        let session = Session {
            app: app.clone(),
            item_id: launch.item.id,
            file_index: launch.video.file_index,
            show_id: launch.item.tmdb_id.filter(|_| launch.item.kind.as_deref() == Some("tv")),
            state,
            tracks: launch.tracks,
            first_selection: None,
            tracks_changed: false,
            after_load,
            last_save: Instant::now(),
            save_soon: false,
            last_emit: Instant::now(),
            stopping: false,
            ended: false,
            want_next: false,
            next: None,
            countdown: None,
        };
        session.emit();
        tauri::async_runtime::spawn(run(session, stream, command_rx, child, stop_rx, generation, socket));
        Ok(())
    }
}

fn launch_label(launch: &Launch) -> String {
    match launch.video.season {
        Some(s) => format!("{} · {}", launch.item.title, episode_label(s, launch.video.episode)),
        None => launch.item.title.clone(),
    }
}

fn is_episode(launch: &Launch) -> bool {
    launch.video.episode.is_some()
}

fn switch_commands(launch: &Launch) -> (Vec<Value>, Vec<Value>) {
    let label = launch_label(launch);
    let cmd = |args: Vec<Value>| json!({ "command": args });
    let mut before = vec![
        cmd(vec!["set".into(), "start".into(), format!("{:.1}", launch.start.unwrap_or(0.0)).into()]),
        cmd(vec!["set".into(), "force-media-title".into(), label.clone().into()]),
        cmd(vec!["set".into(), "title".into(), format!("{label} · cardamoi").into()]),
    ];
    let languages = slang(launch);
    if !languages.is_empty() {
        before.push(cmd(vec!["set".into(), "slang".into(), languages.join(",").into()]));
    }
    if let Some(lang) = &launch.tracks.audio_lang {
        before.push(cmd(vec!["set".into(), "alang".into(), lang.clone().into()]));
    }
    before.push(cmd(vec!["loadfile".into(), launch.source.clone().into(), "replace".into()]));

    let mut after = vec![cmd(vec![
        "set".into(),
        "sub-delay".into(),
        (launch.tracks.sub_offset_ms as f64 / 1000.0).into(),
    ])];
    for file in &launch.subtitle_files {
        after.push(cmd(vec!["sub-add".into(), file.clone().into(), "auto".into()]));
    }
    if launch.tracks.sub_lang.as_deref() == Some("off") {
        after.push(cmd(vec!["set".into(), "sid".into(), "no".into()]));
    }
    let text = match launch.start {
        Some(s) => format!("{label}\nResumed at {} · Press Home to start over", clock(s)),
        None => label,
    };
    after.push(cmd(vec!["show-text".into(), text.into(), 4000.into()]));
    (before, after)
}

fn slang(launch: &Launch) -> Vec<&str> {
    let mut languages: Vec<&str> = vec![];
    if let Some(lang) = launch.tracks.sub_lang.as_deref().filter(|l| *l != "off") {
        languages.push(lang);
    }
    for lang in &launch.languages {
        if !languages.contains(&lang.as_str()) {
            languages.push(lang);
        }
    }
    languages
}

fn episode_label(season: u32, episode: Option<u32>) -> String {
    match episode {
        Some(e) => format!("S{season:02}E{e:02}"),
        None => format!("Season {season}"),
    }
}

fn osd_message(start: Option<f64>, show_hint: bool) -> Option<String> {
    let resumed = start.map(|s| format!("Resumed at {} · Press Home to start over", clock(s)));
    let hint = show_hint.then(|| "Space pause · ←/→ 10 s · C subtitles · F full screen".to_owned());
    match (resumed, hint) {
        (Some(r), Some(h)) => Some(format!("{r}\n{h}")),
        (r, h) => r.or(h),
    }
}

fn mpv_args(launch: &Launch, label: &str, socket: &Path, input_conf: &Path) -> Vec<String> {
    let mut args = vec![
        launch.source.clone(),
        format!("--input-ipc-server={}", socket.display()),
        format!("--input-conf={}", input_conf.display()),
        format!("--force-media-title={label}"),
        format!("--title={label} · cardamoi"),
        "--force-window=immediate".into(),
        // Keep episodes open at the end for autoplay.
        format!("--keep-open={}", if is_episode(launch) { "yes" } else { "no" }),
        "--idle=no".into(),
        "--no-terminal".into(),
        "--hwdec=auto-safe".into(),
        "--sub-auto=fuzzy".into(),
        "--subs-with-matching-audio=forced".into(),
        // cardamoi tracks positions, not mpv.
        "--no-resume-playback".into(),
        "--save-position-on-quit=no".into(),
        "--cache=yes".into(),
        "--demuxer-max-bytes=150MiB".into(),
    ];
    if let Some(start) = launch.start {
        args.push(format!("--start={start:.1}"));
    }
    if launch.tracks.sub_lang.as_deref() == Some("off") {
        args.push("--sid=no".into());
    }
    let languages = slang(launch);
    if !languages.is_empty() {
        args.push(format!("--slang={}", languages.join(",")));
    }
    if let Some(lang) = &launch.tracks.audio_lang {
        args.push(format!("--alang={lang}"));
    }
    for file in &launch.subtitle_files {
        args.push(format!("--sub-files-append={file}"));
    }
    if launch.tracks.sub_offset_ms != 0 {
        args.push(format!("--sub-delay={}", launch.tracks.sub_offset_ms as f64 / 1000.0));
    }
    args
}

fn find_mpv(app: &AppHandle) -> Option<PathBuf> {
    let exe = if cfg!(windows) { "mpv.exe" } else { "mpv" };
    let bundled = app.path().resource_dir().ok().map(|d| d.join("mpv").join(exe));
    let on_path: Vec<_> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join(exe)).collect())
        .unwrap_or_default();
    // Finder-launched apps don't get the shell's PATH.
    let usual = ["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv", "/Applications/mpv.app/Contents/MacOS/mpv", "/usr/bin/mpv"]
        .map(PathBuf::from);
    bundled.into_iter().chain(on_path).chain(usual).find(|p| p.is_file())
}

#[cfg(unix)]
fn ipc_path(data_dir: &Path) -> PathBuf {
    data_dir.join(format!("mpv-{}.sock", &uuid::Uuid::new_v4().simple().to_string()[..8]))
}

#[cfg(windows)]
fn ipc_path(_data_dir: &Path) -> PathBuf {
    PathBuf::from(format!(r"\\.\pipe\cardamoi-mpv-{}", uuid::Uuid::new_v4().simple()))
}

#[cfg(unix)]
async fn connect(path: &Path) -> std::io::Result<tokio::net::UnixStream> {
    tokio::net::UnixStream::connect(path).await
}

#[cfg(windows)]
async fn connect(path: &Path) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    tokio::net::windows::named_pipe::ClientOptions::new().open(path)
}

fn with_db(app: &AppHandle, f: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<()>) {
    let state = app.state::<AppState>();
    if let Err(e) = f(&state.db.lock().unwrap()) {
        eprintln!("player: couldn't save playback state: {e}");
    }
}

struct Session {
    app: AppHandle,
    item_id: i64,
    file_index: i64,
    show_id: Option<i64>,
    state: Arc<Mutex<PlayerState>>,
    tracks: db::TrackPrefs,
    first_selection: Option<(Option<i64>, Option<i64>)>,
    tracks_changed: bool,
    after_load: Vec<Value>,
    last_save: Instant,
    save_soon: bool,
    last_emit: Instant,
    stopping: bool,
    ended: bool,
    want_next: bool,
    next: Option<Launch>,
    countdown: Option<Countdown>,
}

struct Countdown {
    left: u32,
    tick: tokio::time::Instant,
}

impl Session {
    fn emit(&self) {
        let state = self.state.lock().unwrap().clone();
        let _ = self.app.emit("player://state", state);
    }

    fn save_position(&mut self) {
        let (position, duration) = {
            let s = self.state.lock().unwrap();
            (s.position, s.duration)
        };
        with_db(&self.app, |db| db::save_position(db, self.item_id, self.file_index, position, duration));
        self.last_save = Instant::now();
        self.save_soon = false;
    }

    fn mark_watched(&mut self) {
        let mut s = self.state.lock().unwrap();
        if !s.watched {
            s.watched = true;
            with_db(&self.app, |db| db::set_watched(db, self.item_id, self.file_index, true));
        }
    }

    fn handle(&mut self, msg: &Value) -> Vec<Value> {
        let mut replies = vec![];
        let mut emit_now = true;
        match msg.get("event").and_then(Value::as_str) {
            Some("property-change") => {
                let data = &msg["data"];
                match msg["name"].as_str().unwrap_or_default() {
                    "time-pos" => {
                        let Some(position) = data.as_f64() else { return replies };
                        let duration = {
                            let mut s = self.state.lock().unwrap();
                            s.position = position;
                            s.duration
                        };
                        if is_watched(position, duration) {
                            self.mark_watched();
                        }
                        if self.save_soon || self.last_save.elapsed() >= SAVE_EVERY {
                            self.save_position();
                        }
                        emit_now = self.last_emit.elapsed() >= EMIT_EVERY;
                    }
                    "duration" => self.state.lock().unwrap().duration = data.as_f64(),
                    "pause" => {
                        self.state.lock().unwrap().paused = data.as_bool().unwrap_or(false);
                        self.save_position();
                    }
                    "paused-for-cache" => self.state.lock().unwrap().buffering = data.as_bool().unwrap_or(false),
                    "track-list" => self.set_tracks(data),
                    "sid" | "aid" => {
                        replies.push(json!({ "command": ["get_property", "track-list"], "request_id": TRACK_LIST_REQUEST }));
                    }
                    "eof-reached" if data.as_bool() == Some(true) && !self.ended => {
                        self.ended = true;
                        self.want_next = true;
                        self.state.lock().unwrap().status = "ended";
                        self.mark_watched();
                        self.save_position();
                    }
                    "eof-reached" if data.as_bool() == Some(false) && self.ended => {
                        self.ended = false;
                        self.countdown = None;
                        self.next = None;
                        self.state.lock().unwrap().status = "playing";
                    }
                    "sub-delay" => {
                        let delay = data.as_f64().unwrap_or(0.0);
                        self.state.lock().unwrap().sub_delay = delay;
                        let offset = (delay * 1000.0).round() as i64;
                        if offset != self.tracks.sub_offset_ms {
                            self.tracks.sub_offset_ms = offset;
                            self.save_tracks();
                        }
                    }
                    _ => emit_now = false,
                }
            }
            Some("file-loaded") => {
                self.state.lock().unwrap().status = "playing";
                replies.append(&mut self.after_load);
            }
            Some("client-message") => match msg["args"][0].as_str() {
                Some("cardamoi-cancel") if self.countdown.is_some() => {
                    self.countdown = None;
                    self.next = None;
                    replies.push(json!({ "command": ["show-text", "Autoplay cancelled", 2000] }));
                }
                Some("cardamoi-next") if self.countdown.is_some() => {
                    if let Some(countdown) = &mut self.countdown {
                        countdown.left = 0;
                        countdown.tick = tokio::time::Instant::now();
                    }
                }
                _ => emit_now = false,
            },
            Some("seek") => self.save_soon = true,
            Some("end-file") => {
                let mut s = self.state.lock().unwrap();
                match msg["reason"].as_str() {
                    Some("eof") => {
                        s.status = "ended";
                        drop(s);
                        self.mark_watched();
                    }
                    Some("error") => {
                        s.status = "failed";
                        s.error = Some(match msg["file_error"].as_str() {
                            Some(e) => format!("mpv couldn't play this video ({e})."),
                            None => "mpv couldn't play this video.".into(),
                        });
                    }
                    _ => {}
                }
            }
            None if msg["request_id"] == TRACK_LIST_REQUEST => self.set_tracks(&msg["data"]),
            _ => emit_now = false,
        }
        if emit_now {
            self.last_emit = Instant::now();
            self.emit();
        }
        replies
    }

    fn set_tracks(&mut self, data: &Value) {
        let Ok(tracks) = serde_json::from_value::<Vec<Track>>(data.clone()) else { return };
        let selected = |kind: &str| tracks.iter().find(|t| t.kind == kind && t.selected);
        let selection = (selected("sub").map(|t| t.id), selected("audio").map(|t| t.id));
        let loaded = tracks.iter().any(|t| t.kind == "video");

        let mut prefs = self.tracks.clone();
        if tracks.iter().any(|t| t.kind == "sub") {
            match selected("sub") {
                Some(t) => prefs.sub_lang = t.lang.clone().or(prefs.sub_lang),
                None => prefs.sub_lang = Some("off".into()),
            }
        }
        if let Some(lang) = selected("audio").and_then(|t| t.lang.clone()) {
            if tracks.iter().filter(|t| t.kind == "audio").count() > 1 {
                prefs.audio_lang = Some(lang);
            }
        }
        self.state.lock().unwrap().tracks = tracks;

        if !loaded {
            return;
        }
        match self.first_selection {
            None => self.first_selection = Some(selection),
            Some(first) if first != selection => self.tracks_changed = true,
            _ => {}
        }
        if self.tracks_changed && prefs != self.tracks {
            self.tracks = prefs;
            self.save_tracks();
        }
    }

    fn save_tracks(&self) {
        with_db(&self.app, |db| {
            db::save_tracks(db, self.item_id, self.file_index, &self.tracks)?;
            match self.show_id {
                Some(show) => db::save_show_tracks(db, show, &self.tracks),
                None => Ok(()),
            }
        });
    }

    async fn look_up_next(&mut self) -> Vec<Value> {
        if self.stopping {
            return vec![];
        }
        let state = self.app.state::<AppState>();
        let autoplay = db::get_setting(&state.db.lock().unwrap(), "autoplay_next").ok().flatten().as_deref() != Some("0");
        if !autoplay || self.show_id.is_none() {
            return vec![];
        }
        match crate::next_episode_launch(&state, self.item_id, self.file_index).await {
            Some(launch) => {
                self.next = Some(launch);
                self.countdown = Some(Countdown { left: AUTOPLAY_SECS, tick: tokio::time::Instant::now() });
                vec![]
            }
            None => vec![json!({ "command": ["show-text", "That was the last episode you have. Find the next one in cardamoi.", 6000] })],
        }
    }

    fn count_down(&mut self) -> Vec<Value> {
        let Some(countdown) = &mut self.countdown else { return vec![] };
        if countdown.left > 0 {
            let label = self.next.as_ref().map(launch_label).unwrap_or_default();
            let text = format!("Next: {label} in {} s\nEsc to cancel · Enter to play now", countdown.left);
            countdown.left -= 1;
            countdown.tick += Duration::from_secs(1);
            return vec![json!({ "command": ["show-text", text, 1500] })];
        }
        self.countdown = None;
        match self.next.take() {
            Some(launch) => self.switch_to(launch),
            None => vec![],
        }
    }

    fn switch_to(&mut self, launch: Launch) -> Vec<Value> {
        self.save_position();
        let (before, after) = switch_commands(&launch);
        *self.state.lock().unwrap() = PlayerState {
            item_id: launch.item.id,
            file_index: launch.video.file_index,
            title: launch_label(&launch),
            status: "opening",
            position: launch.start.unwrap_or(0.0),
            duration: launch.video.duration_secs,
            watched: launch.video.watched,
            ..Default::default()
        };
        self.item_id = launch.item.id;
        self.file_index = launch.video.file_index;
        self.show_id = launch.item.tmdb_id.filter(|_| launch.item.kind.as_deref() == Some("tv"));
        self.tracks = launch.tracks;
        self.first_selection = None;
        self.tracks_changed = false;
        self.after_load = after;
        self.ended = false;
        self.save_soon = false;
        self.emit();
        before
    }

    fn finish(mut self, exit: Option<ExitStatus>) {
        self.save_position();
        {
            let mut s = self.state.lock().unwrap();
            s.buffering = false;
            if !matches!(s.status, "ended" | "failed") {
                s.status = match exit {
                    _ if self.stopping => "stopped",
                    Some(status) if status.success() => "stopped",
                    Some(status) if status.code().is_some() => "failed",
                    _ => "crashed",
                };
                match s.status {
                    "failed" => s.error = Some("mpv couldn't play this video.".into()),
                    "crashed" => s.error = Some(format!("The player closed unexpectedly. Your place ({}) was saved.", clock(s.position))),
                    _ => {}
                }
            }
        }
        self.emit();
    }
}

async fn run<S: AsyncRead + AsyncWrite + Send + 'static>(
    mut session: Session,
    stream: S,
    mut commands: mpsc::UnboundedReceiver<Value>,
    mut child: Child,
    mut stop: oneshot::Receiver<()>,
    generation: u64,
    socket: PathBuf,
) {
    let (read, mut write) = tokio::io::split(stream);
    let mut lines = BufReader::new(read).lines();
    for (id, property) in
        ["time-pos", "duration", "pause", "paused-for-cache", "track-list", "sid", "aid", "sub-delay", "eof-reached"]
            .iter()
            .enumerate()
    {
        let _ = send(&mut write, &json!({ "command": ["observe_property", id + 1, property] })).await;
    }

    let mut exit = None;
    loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
                    for reply in session.handle(&msg) {
                        let _ = send(&mut write, &reply).await;
                    }
                }
                _ => break,
            },
            Some(command) = commands.recv() => {
                let _ = send(&mut write, &command).await;
            }
            status = child.wait() => {
                exit = status.ok();
                break;
            }
            _ = &mut stop, if !session.stopping => {
                session.stopping = true;
                let _ = send(&mut write, &json!({ "command": ["quit"] })).await;
            }
            _ = tokio::time::sleep_until(session.countdown.as_ref().map_or_else(tokio::time::Instant::now, |c| c.tick)),
                if session.countdown.is_some() =>
            {
                for command in session.count_down() {
                    let _ = send(&mut write, &command).await;
                }
            }
        }
        if session.want_next {
            session.want_next = false;
            for command in session.look_up_next().await {
                let _ = send(&mut write, &command).await;
            }
        }
    }
    if exit.is_none() {
        exit = match tokio::time::timeout(Duration::from_secs(2), child.wait()).await {
            Ok(Ok(status)) => Some(status),
            _ => {
                let _ = child.start_kill();
                None
            }
        };
    }
    #[cfg(unix)]
    let _ = std::fs::remove_file(&socket);
    #[cfg(windows)]
    let _ = socket;

    let app = session.app.clone();
    session.finish(exit);
    app.state::<AppState>().player.clear(generation);
}

async fn send<W: AsyncWrite + Unpin>(write: &mut W, msg: &Value) -> std::io::Result<()> {
    let mut line = msg.to_string();
    line.push('\n');
    write.write_all(line.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resumes_between_the_start_and_the_credits() {
        assert_eq!(resume_from(10.0, Some(6000.0)), None);
        assert_eq!(resume_from(2467.0, Some(6000.0)), Some(2462.0));
        assert_eq!(resume_from(5500.0, Some(6000.0)), None);
        assert_eq!(resume_from(2467.0, None), Some(2462.0));
    }

    #[test]
    fn watched_after_the_credits_start() {
        assert!(!is_watched(5000.0, Some(6000.0)));
        assert!(is_watched(5400.0, Some(6000.0)));
        assert!(!is_watched(5400.0, None));
        assert!(!is_watched(0.0, Some(0.0)));
    }

    #[test]
    fn buffers_five_percent_up_to_50_mib() {
        assert_eq!(buffer_target(300 * 1024 * 1024), 15 * 1024 * 1024);
        assert_eq!(buffer_target(4 * 1024 * 1024 * 1024), 50 * 1024 * 1024);
    }

    #[test]
    fn clocks() {
        assert_eq!(clock(67.9), "1:07");
        assert_eq!(clock(2467.0), "41:07");
        assert_eq!(clock(3723.0), "1:02:03");
    }

    fn launch(tracks: db::TrackPrefs, languages: &[&str]) -> Launch {
        Launch {
            item: db::Item {
                id: 1,
                title: "Dune".into(),
                year: Some(2021),
                source: "x".into(),
                info_hash: "aa".into(),
                poster: None,
                added_at: 0,
                tmdb_id: None,
                kind: None,
                season: None,
                episode: None,
                torrent_url: None,
                single_video: false,
                local_path: None,
            },
            video: db::Video { item_id: 1, file_index: 0, path: "dune.mkv".into(), ..Default::default() },
            source: "/films/dune.mkv".into(),
            start: Some(2462.0),
            tracks,
            languages: languages.iter().map(|l| l.to_string()).collect(),
            show_hint: false,
            subtitle_files: vec!["/films/Subs/2_English.srt".into()],
        }
    }

    fn args(l: &Launch) -> Vec<String> {
        mpv_args(l, "Dune", Path::new("/tmp/mpv.sock"), Path::new("/tmp/input.conf"))
    }

    #[test]
    fn remembered_subtitle_language_comes_first() {
        let prefs = db::TrackPrefs { sub_lang: Some("ar".into()), audio_lang: Some("ja".into()), sub_offset_ms: -500 };
        let a = args(&launch(prefs, &["en", "ar"]));
        assert_eq!(a[0], "/films/dune.mkv");
        assert!(a.contains(&"--start=2462.0".into()));
        assert!(a.contains(&"--slang=ar,en".into()));
        assert!(a.contains(&"--alang=ja".into()));
        assert!(a.contains(&"--sub-delay=-0.5".into()));
        assert!(a.contains(&"--sub-files-append=/films/Subs/2_English.srt".into()));
    }

    #[test]
    fn subtitles_turned_off_stay_off() {
        let prefs = db::TrackPrefs { sub_lang: Some("off".into()), ..Default::default() };
        let a = args(&launch(prefs, &["en"]));
        assert!(a.contains(&"--sid=no".into()));
        assert!(a.contains(&"--slang=en".into()));
        assert!(!a.iter().any(|a| a.starts_with("--sub-delay")));
    }

    #[test]
    fn episodes_stay_open_at_the_end_and_films_close() {
        let mut film = launch(db::TrackPrefs::default(), &[]);
        assert!(args(&film).contains(&"--keep-open=no".into()));
        film.video.episode = Some(3);
        assert!(args(&film).contains(&"--keep-open=yes".into()));
    }

    #[test]
    fn switching_to_the_next_episode() {
        let prefs = db::TrackPrefs { sub_lang: Some("off".into()), audio_lang: None, sub_offset_ms: 500 };
        let mut next = launch(prefs, &["en"]);
        next.item.title = "Severance".into();
        next.video.season = Some(2);
        next.video.episode = Some(5);
        next.start = None;
        let (before, after) = switch_commands(&next);
        let names: Vec<String> = before.iter().map(|c| c["command"][0].as_str().unwrap().to_owned() + " " + c["command"][1].as_str().unwrap()).collect();
        assert_eq!(names, ["set start", "set force-media-title", "set title", "set slang", "loadfile /films/dune.mkv"]);
        assert_eq!(before[0]["command"][2], "0.0");
        assert_eq!(before[1]["command"][2], "Severance · S02E05");
        assert_eq!(after[0]["command"], json!(["set", "sub-delay", 0.5]));
        assert_eq!(after[1]["command"], json!(["sub-add", "/films/Subs/2_English.srt", "auto"]));
        assert_eq!(after[2]["command"], json!(["set", "sid", "no"]));
        assert_eq!(after[3]["command"][1], "Severance · S02E05");
    }

    #[test]
    fn osd_tells_where_playback_resumed() {
        assert_eq!(osd_message(Some(2467.0), false).unwrap(), "Resumed at 41:07 · Press Home to start over");
        assert!(osd_message(None, true).unwrap().starts_with("Space pause"));
        assert_eq!(osd_message(None, false), None);
    }

    #[test]
    fn reads_mpv_track_lists() {
        let data = json!([
            { "id": 1, "type": "video", "selected": true },
            { "id": 1, "type": "sub", "lang": "en", "title": "SDH", "hearing-impaired": true, "external": false, "selected": true },
        ]);
        let tracks: Vec<Track> = serde_json::from_value(data).unwrap();
        assert_eq!(tracks[1].kind, "sub");
        assert!(tracks[1].hearing_impaired && tracks[1].selected);
        assert_eq!(tracks[0].lang, None);
    }
}
