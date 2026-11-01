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

use crate::{AppState, db, libmpv::{self, Mpv}, surface::Surface};

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

/// Where the picture is and who draws the controls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    /// The built-in libmpv, drawing in the app window; the app draws the controls.
    Embedded,
    /// The built-in libmpv in a window of its own, with mpv's keys and on-screen text.
    Window,
    /// The user's own mpv, in a window of its own.
    #[default]
    External,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextUp {
    pub title: String,
    pub seconds_left: u32,
}

/// Picks how to play. `own`: the user chose their own mpv; `libmpv`: the built-in library loads;
/// `embeds`: this platform can draw in the app window; `mpv_found`: an mpv is installed.
pub fn resolve_mode(own: bool, libmpv: bool, embeds: bool, mpv_found: bool) -> Result<Mode, PlayError> {
    match (own, libmpv, embeds, mpv_found) {
        (true, _, _, true) => Ok(Mode::External),
        (true, _, _, false) => Err(PlayError::MpvMissing),
        (false, true, true, _) => Ok(Mode::Embedded),
        (false, true, false, _) => Ok(Mode::Window),
        (false, false, _, true) => Ok(Mode::External),
        (false, false, _, false) => Err(PlayError::PlayerMissing),
    }
}

/// Resolves the mode for this machine and the `player` setting.
pub fn mode_for(app: &AppHandle, own: bool) -> Result<Mode, PlayError> {
    let libmpv = app.path().resource_dir().is_ok_and(|dir| libmpv::available(&dir));
    let embeds = crate::surface::EMBEDS && app.try_state::<Surface>().is_some();
    resolve_mode(own, libmpv, embeds, find_mpv().is_some())
}

/// How a player ended.
#[derive(Debug, PartialEq)]
enum Exit {
    Clean,
    Failed,
    Crashed,
}

fn exit_of(status: Option<ExitStatus>) -> Exit {
    match status {
        Some(s) if s.success() => Exit::Clean,
        Some(s) if s.code().is_some() => Exit::Failed,
        _ => Exit::Crashed,
    }
}

/// What plays the video: mpv as a process of its own, or the libmpv built into the app.
enum Engine {
    External(Child),
    #[allow(dead_code)] // started by the built-in modes
    Builtin(Mpv, oneshot::Receiver<()>),
}

impl Engine {
    async fn wait(&mut self) -> Exit {
        match self {
            Engine::External(child) => exit_of(child.wait().await.ok()),
            Engine::Builtin(_, shutdown) => {
                let _ = shutdown.await;
                Exit::Clean
            }
        }
    }

    fn kill(&mut self) {
        if let Engine::External(child) = self {
            let _ = child.start_kill();
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlayError {
    MpvMissing,
    PlayerMissing,
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
    pub mode: Mode,
    /// Where a resumed start picked up, for the app's own "Resumed at" line.
    pub resumed_from: Option<f64>,
    pub buffer_percent: Option<f64>,
    /// 0 to 100, as mpv has it.
    pub volume: f64,
    pub muted: bool,
    /// How far ahead of the playhead the cache reaches, in seconds of the file.
    pub cache_until: Option<f64>,
    pub next_up: Option<NextUp>,
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
        const COMMANDS: &[&str] =
            &["sub-add", "sub-remove", "sub-reload", "set", "cycle", "add", "seek", "show-text", "script-message"];
        const PROPERTIES: &[&str] = &["sid", "aid", "sub", "sub-delay", "sub-visibility", "pause", "volume", "mute", "fullscreen"];
        const MESSAGES: &[&str] = &["cardamoi-next", "cardamoi-cancel"];
        let name = args.first().and_then(Value::as_str).unwrap_or_default();
        anyhow::ensure!(COMMANDS.contains(&name), "the player doesn't accept \"{name}\"");
        if matches!(name, "set" | "cycle" | "add") {
            let property = args.get(1).and_then(Value::as_str).unwrap_or_default();
            anyhow::ensure!(PROPERTIES.contains(&property), "the player doesn't let \"{property}\" be changed");
        }
        if name == "script-message" {
            let message = args.get(1).and_then(Value::as_str).unwrap_or_default();
            anyhow::ensure!(MESSAGES.contains(&message), "the player doesn't accept the message \"{message}\"");
        }
        let current = self.current.lock().unwrap();
        let live = current.as_ref().ok_or_else(|| anyhow::anyhow!("nothing is playing"))?;
        live.commands.send(json!({ "command": args }))?;
        Ok(())
    }

    pub fn stop(&self, app: &AppHandle) {
        let Some(mut live) = self.current.lock().unwrap().take() else { return };
        let state = live.state.lock().unwrap().clone();
        with_db(Some(app), |db| db::save_position(db, state.item_id, state.file_index, state.position, state.duration));
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

    pub async fn start(&self, app: &AppHandle, launch: Launch, mode: Mode) -> Result<(), PlayError> {
        self.stop(app);
        let data_dir = app.path().app_data_dir()?;
        let input_conf = data_dir.join("mpv-input.conf");
        std::fs::write(&input_conf, INPUT_CONF)?;
        let socket = ipc_path(&data_dir);
        let label = launch_label(&launch);
        let embedded = mode == Mode::Embedded;

        let (engine, stream, first) = match mode {
            Mode::External => {
                let mpv = find_mpv().ok_or(PlayError::MpvMissing)?;
                let mut child = tokio::process::Command::new(&mpv)
                    .args(mpv_args(&launch, &label, &socket, &input_conf))
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(|e| format!("couldn't start mpv ({}): {e}", mpv.display()))?;
                let stream = connect_ipc(&socket, || match child.try_wait() {
                    Ok(Some(status)) => Some(format!("mpv closed before playing ({status})")),
                    _ => None,
                })
                .await;
                match stream {
                    Ok(stream) => (Engine::External(child), stream, vec![]),
                    Err(e) => {
                        let _ = child.start_kill();
                        return Err(e);
                    }
                }
            }
            Mode::Embedded | Mode::Window => {
                let mut mpv = Mpv::load(&libmpv::candidates(&app.path().resource_dir()?))?;
                for (name, value) in builtin_options(&launch, &label, &socket, &input_conf, embedded) {
                    mpv.set_option(&name, &value)?;
                }
                mpv.initialize()?;
                if embedded {
                    let surface = app.try_state::<Surface>().ok_or("the video view isn't set up")?;
                    surface.connect(&mut mpv)?;
                    surface.set_visible(app, true);
                }
                let shutdown = mpv.watch_shutdown();
                let stream = connect_ipc(&socket, || None).await?;
                // Loaded once the session is listening, so no early event is missed.
                let load = json!({ "command": ["loadfile", launch.source.clone()] });
                (Engine::Builtin(mpv, shutdown), stream, vec![load])
            }
        };

        let state = Arc::new(Mutex::new(initial_state(&launch, label, mode)));
        let (commands, command_rx) = mpsc::unbounded_channel();
        let (stop, stop_rx) = oneshot::channel();
        let generation = self.generation.fetch_add(1, Ordering::Relaxed);
        *self.current.lock().unwrap() = Some(Live { generation, commands, state: state.clone(), stop: Some(stop) });

        let session = Session {
            app: Some(app.clone()),
            osd: !embedded,
            embedded,
            builtin: mode != Mode::External,
            quit_on_eof: mode != Mode::External && !is_episode(&launch),
            item_id: launch.item.id,
            file_index: launch.video.file_index,
            show_id: launch.item.tmdb_id.filter(|_| launch.item.kind.as_deref() == Some("tv")),
            state,
            tracks: launch.tracks.clone(),
            first_selection: None,
            tracks_changed: false,
            after_load: after_load_commands(&launch, !embedded),
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
        tauri::async_runtime::spawn(run(session, stream, command_rx, engine, first, stop_rx, generation, socket));
        Ok(())
    }
}

trait Io: AsyncRead + AsyncWrite + Send + Unpin + 'static {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin + 'static> Io for T {}

/// Connects to mpv's IPC socket, which it creates shortly after starting. `dead` says why mpv is gone, if it is.
async fn connect_ipc(
    socket: &Path,
    mut dead: impl FnMut() -> Option<String>,
) -> Result<Box<dyn Io>, PlayError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match connect(socket).await {
            Ok(stream) => return Ok(Box::new(stream)),
            Err(_) if Instant::now() < deadline => {
                if let Some(why) = dead() {
                    return Err(why.into());
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Err(e) => return Err(format!("couldn't connect to mpv: {e}").into()),
        }
    }
}

/// The state a session starts in; the embedded player shows resumes itself.
fn initial_state(launch: &Launch, title: String, mode: Mode) -> PlayerState {
    PlayerState {
        item_id: launch.item.id,
        file_index: launch.video.file_index,
        title,
        status: "opening",
        position: launch.start.unwrap_or(0.0),
        duration: launch.video.duration_secs,
        watched: launch.video.watched,
        mode,
        resumed_from: launch.start.filter(|_| mode == Mode::Embedded),
        ..Default::default()
    }
}

/// What mpv is told once a file has loaded; none when the app shows these things itself.
fn after_load_commands(launch: &Launch, osd: bool) -> Vec<Value> {
    if !osd {
        return vec![];
    }
    osd_message(launch.start, launch.show_hint)
        .map(|text| vec![json!({ "command": ["show-text", text, 4000] })])
        .unwrap_or_default()
}

/// The options for libmpv: what `mpv_args` passes mpv, as names and values. Embedded, the app draws the
/// picture's controls and handles the keys, so mpv gets no bindings, no on-screen display and no window.
fn builtin_options(launch: &Launch, label: &str, socket: &Path, input_conf: &Path, embedded: bool) -> Vec<(String, String)> {
    let mut options: Vec<(String, String)> = mpv_args(launch, label, socket, input_conf)
        .into_iter()
        .skip(1) // the source, which is loaded as a command
        .filter_map(|arg| {
            let arg = arg.strip_prefix("--")?.to_owned();
            Some(match arg.split_once('=') {
                Some((name, value)) => (name.to_owned(), value.to_owned()),
                None => match arg.strip_prefix("no-") {
                    Some(name) => (name.to_owned(), "no".to_owned()),
                    None => (arg, "yes".to_owned()),
                },
            })
        })
        .collect();
    // Nothing is loaded when mpv starts, and with `idle=no` it would quit before the file arrives.
    for (name, value) in &mut options {
        if name == "idle" {
            *value = "yes".into();
        }
    }
    if embedded {
        options.retain(|(name, _)| !matches!(name.as_str(), "input-conf" | "force-window" | "title"));
        for (name, value) in [("vo", "libmpv"), ("input-default-bindings", "no"), ("input-vo-keyboard", "no"), ("osc", "no"), ("osd-level", "0")] {
            options.push((name.into(), value.into()));
        }
    } else {
        // libmpv starts with mpv's keys and on-screen controller off; in its own window it needs them, as the mpv command has.
        for (name, value) in [("input-default-bindings", "yes"), ("input-vo-keyboard", "yes"), ("osc", "yes")] {
            options.push((name.into(), value.into()));
        }
    }
    options
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

/// The user's own mpv.
fn find_mpv() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "mpv.exe" } else { "mpv" };
    let on_path: Vec<_> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|d| d.join(exe)).collect())
        .unwrap_or_default();
    // Finder-launched apps don't get the shell's PATH.
    let usual = ["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv", "/Applications/mpv.app/Contents/MacOS/mpv", "/usr/bin/mpv"]
        .map(PathBuf::from);
    on_path.into_iter().chain(usual).find(|p| p.is_file())
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

fn with_db(app: Option<&AppHandle>, f: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<()>) {
    let Some(app) = app else { return };
    let state = app.state::<AppState>();
    if let Err(e) = f(&state.db.lock().unwrap()) {
        eprintln!("player: couldn't save playback state: {e}");
    }
}

struct Session {
    /// None only in tests.
    app: Option<AppHandle>,
    /// Whether mpv shows its own on-screen text; false when the app's controls do.
    osd: bool,
    embedded: bool,
    /// The built-in libmpv, which stays idle when a file ends and so has to be told to quit.
    builtin: bool,
    /// Quit when the file ends, for films: episodes stay open for the countdown to the next one.
    quit_on_eof: bool,
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
        let Some(app) = &self.app else { return };
        let state = self.state.lock().unwrap().clone();
        let _ = app.emit("player://state", state);
    }

    fn save_position(&mut self) {
        let (position, duration) = {
            let s = self.state.lock().unwrap();
            (s.position, s.duration)
        };
        with_db(self.app.as_ref(), |db| db::save_position(db, self.item_id, self.file_index, position, duration));
        self.last_save = Instant::now();
        self.save_soon = false;
    }

    fn mark_watched(&mut self) {
        let mut s = self.state.lock().unwrap();
        if !s.watched {
            s.watched = true;
            with_db(self.app.as_ref(), |db| db::set_watched(db, self.item_id, self.file_index, true));
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
                    "cache-buffering-state" => self.state.lock().unwrap().buffer_percent = data.as_f64(),
                    "volume" => self.state.lock().unwrap().volume = data.as_f64().unwrap_or(100.0),
                    "mute" => self.state.lock().unwrap().muted = data.as_bool().unwrap_or(false),
                    "demuxer-cache-time" => {
                        self.state.lock().unwrap().cache_until = data.as_f64();
                        emit_now = self.last_emit.elapsed() >= EMIT_EVERY;
                    }
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
                        let mut s = self.state.lock().unwrap();
                        s.status = "playing";
                        s.next_up = None;
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
                    self.state.lock().unwrap().next_up = None;
                    if self.osd {
                        replies.push(json!({ "command": ["show-text", "Autoplay cancelled", 2000] }));
                    }
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
                let reason = msg["reason"].as_str();
                if self.builtin && (reason == Some("error") || (reason == Some("eof") && self.quit_on_eof)) {
                    replies.push(json!({ "command": ["quit"] }));
                }
                let mut s = self.state.lock().unwrap();
                match reason {
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
        if let Some(lang) = selected("audio").and_then(|t| t.lang.clone())
            && tracks.iter().filter(|t| t.kind == "audio").count() > 1
        {
            prefs.audio_lang = Some(lang);
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
        with_db(self.app.as_ref(), |db| {
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
        let Some(app) = self.app.clone() else { return vec![] };
        let state = app.state::<AppState>();
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
            None => {
                let text = "That was the last episode you have. Find the next one in cardamoi.";
                if self.osd {
                    vec![json!({ "command": ["show-text", text, 6000] })]
                } else {
                    self.state.lock().unwrap().error = Some(text.into());
                    self.emit();
                    vec![]
                }
            }
        }
    }

    fn count_down(&mut self) -> Vec<Value> {
        let Some(countdown) = &mut self.countdown else { return vec![] };
        if countdown.left > 0 {
            let label = self.next.as_ref().map(launch_label).unwrap_or_default();
            let seconds_left = countdown.left;
            countdown.left -= 1;
            countdown.tick += Duration::from_secs(1);
            if !self.osd {
                self.state.lock().unwrap().next_up = Some(NextUp { title: label, seconds_left });
                self.emit();
                return vec![];
            }
            let text = format!("Next: {label} in {seconds_left} s\nEsc to cancel · Enter to play now");
            return vec![json!({ "command": ["show-text", text, 1500] })];
        }
        self.countdown = None;
        self.state.lock().unwrap().next_up = None;
        match self.next.take() {
            Some(launch) => self.switch_to(launch),
            None => vec![],
        }
    }

    fn switch_to(&mut self, launch: Launch) -> Vec<Value> {
        self.save_position();
        let (before, mut after) = switch_commands(&launch);
        if !self.osd {
            after.retain(|c| c["command"][0] != "show-text");
        }
        let mode = if self.embedded { Mode::Embedded } else { Mode::Window };
        *self.state.lock().unwrap() = initial_state(&launch, launch_label(&launch), mode);
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

    fn finish(mut self, exit: Exit) {
        self.save_position();
        {
            let mut s = self.state.lock().unwrap();
            s.buffering = false;
            if !matches!(s.status, "ended" | "failed") {
                s.status = match exit {
                    _ if self.stopping => "stopped",
                    Exit::Clean => "stopped",
                    Exit::Failed => "failed",
                    Exit::Crashed => "crashed",
                };
                match s.status {
                    "failed" => s.error = Some("mpv couldn't play this video.".into()),
                    "crashed" => s.error = Some(format!("The player closed unexpectedly. Your place ({}) was saved.", clock(s.position))),
                    _ => {}
                }
            }
        }
        if self.embedded
            && let Some(app) = &self.app
            && let Some(surface) = app.try_state::<Surface>()
        {
            surface.set_visible(app, false);
        }
        self.emit();
    }
}

#[allow(clippy::too_many_arguments)]
async fn run<S: AsyncRead + AsyncWrite + Send + 'static>(
    mut session: Session,
    stream: S,
    mut commands: mpsc::UnboundedReceiver<Value>,
    mut engine: Engine,
    first: Vec<Value>,
    mut stop: oneshot::Receiver<()>,
    generation: u64,
    socket: PathBuf,
) {
    let (read, mut write) = tokio::io::split(stream);
    let mut lines = BufReader::new(read).lines();
    for (id, property) in
        [
            "time-pos",
            "duration",
            "pause",
            "paused-for-cache",
            "track-list",
            "sid",
            "aid",
            "sub-delay",
            "eof-reached",
            "cache-buffering-state",
            "demuxer-cache-time",
            "volume",
            "mute",
        ]
            .iter()
            .enumerate()
    {
        let _ = send(&mut write, &json!({ "command": ["observe_property", id + 1, property] })).await;
    }
    for command in first {
        let _ = send(&mut write, &command).await;
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
            status = engine.wait() => {
                exit = Some(status);
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
        exit = match tokio::time::timeout(Duration::from_secs(2), engine.wait()).await {
            Ok(status) => Some(status),
            Err(_) => {
                engine.kill();
                None
            }
        };
    }
    #[cfg(unix)]
    let _ = std::fs::remove_file(&socket);
    #[cfg(windows)]
    let _ = socket;

    // The built-in player is destroyed off the async threads: tearing down libmpv can take a moment.
    let _ = tokio::task::spawn_blocking(move || drop(engine)).await;
    let app = session.app.clone();
    session.finish(exit.unwrap_or(Exit::Crashed));
    if let Some(app) = app {
        app.state::<AppState>().player.clear(generation);
    }
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
    fn exit_codes_map_to_outcomes() {
        assert_eq!(exit_of(None), Exit::Crashed);
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(exit_of(Some(ExitStatus::from_raw(0))), Exit::Clean);
            assert_eq!(exit_of(Some(ExitStatus::from_raw(2 << 8))), Exit::Failed);
            assert_eq!(exit_of(Some(ExitStatus::from_raw(9))), Exit::Crashed);
        }
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
    fn resolve_mode_prefers_the_built_in_player() {
        assert_eq!(resolve_mode(false, true, true, false).unwrap(), Mode::Embedded);
        assert_eq!(resolve_mode(false, true, false, false).unwrap(), Mode::Window);
        assert_eq!(resolve_mode(false, false, true, true).unwrap(), Mode::External);
        assert!(matches!(resolve_mode(false, false, true, false), Err(PlayError::PlayerMissing)));
    }

    #[test]
    fn resolve_mode_respects_own_mpv() {
        assert_eq!(resolve_mode(true, true, true, true).unwrap(), Mode::External);
        assert!(matches!(resolve_mode(true, true, true, false), Err(PlayError::MpvMissing)));
    }

    fn options(embedded: bool) -> Vec<(String, String)> {
        builtin_options(&launch(db::TrackPrefs::default(), &["en"]), "Dune", Path::new("/s"), Path::new("/i"), embedded)
    }

    fn get<'a>(o: &'a [(String, String)], k: &str) -> Option<&'a str> {
        o.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str())
    }

    #[test]
    fn embedded_options_hand_the_keys_to_the_app() {
        let o = options(true);
        assert_eq!(get(&o, "vo"), Some("libmpv"));
        assert_eq!(get(&o, "input-default-bindings"), Some("no"));
        assert_eq!(get(&o, "osc"), Some("no"));
        assert_eq!(get(&o, "osd-level"), Some("0"));
        assert_eq!(get(&o, "input-ipc-server"), Some("/s"));
        assert_eq!(get(&o, "start"), Some("2462.0"));
        assert_eq!(get(&o, "slang"), Some("en"));
        assert_eq!(get(&o, "terminal"), Some("no"));
        assert!(get(&o, "input-conf").is_none() && get(&o, "force-window").is_none() && get(&o, "title").is_none());
    }

    #[test]
    fn built_in_players_wait_for_a_file() {
        // With nothing loaded at start, mpv would quit before the file is sent.
        assert_eq!(get(&options(true), "idle"), Some("yes"));
        assert_eq!(get(&options(false), "idle"), Some("yes"));
    }

    #[test]
    fn built_in_films_quit_when_they_end_and_episodes_stay() {
        let end = json!({ "event": "end-file", "reason": "eof" });
        let quits = |replies: &[Value]| replies.iter().any(|c| c["command"][0] == "quit");
        let mut film = test_session(false);
        film.builtin = true;
        film.quit_on_eof = true;
        assert!(quits(&film.handle(&end)));
        assert_eq!(film.state.lock().unwrap().status, "ended");
        let mut episode = test_session(false);
        episode.builtin = true;
        assert!(!quits(&episode.handle(&end)));
        let mut external = test_session(true);
        external.quit_on_eof = true;
        assert!(!quits(&external.handle(&end)), "an external mpv quits by itself");
    }

    #[test]
    fn built_in_players_quit_after_an_error() {
        let mut s = test_session(false);
        s.builtin = true;
        let replies = s.handle(&json!({ "event": "end-file", "reason": "error", "file_error": "unsupported format" }));
        assert!(replies.iter().any(|c| c["command"][0] == "quit"));
        assert_eq!(s.state.lock().unwrap().status, "failed");
    }

    #[test]
    fn window_mode_keeps_mpv_keys_and_window() {
        let o = options(false);
        assert_eq!(get(&o, "input-conf"), Some("/i"));
        assert_eq!(get(&o, "force-window"), Some("immediate"));
        assert!(get(&o, "vo").is_none());
        // libmpv starts with these off, unlike the mpv command; its own window needs them.
        assert_eq!(get(&o, "input-default-bindings"), Some("yes"));
        assert_eq!(get(&o, "input-vo-keyboard"), Some("yes"));
        assert_eq!(get(&o, "osc"), Some("yes"));
    }

    #[test]
    fn only_our_script_messages_are_allowed() {
        let p = Player::default();
        let err = p.command(vec!["script-message".into(), "run".into()]).unwrap_err();
        assert!(err.to_string().contains("doesn't accept"));
        // An allowed message gets as far as "nothing is playing".
        let err = p.command(vec!["script-message".into(), "cardamoi-next".into()]).unwrap_err();
        assert!(err.to_string().contains("nothing is playing"));
    }

    fn test_session(osd: bool) -> Session {
        Session {
            app: None,
            osd,
            embedded: !osd,
            builtin: false,
            quit_on_eof: false,
            item_id: 1,
            file_index: 0,
            show_id: Some(95396),
            state: Arc::new(Mutex::new(PlayerState::default())),
            tracks: db::TrackPrefs::default(),
            first_selection: None,
            tracks_changed: false,
            after_load: vec![],
            last_save: Instant::now(),
            save_soon: false,
            last_emit: Instant::now(),
            stopping: false,
            ended: false,
            want_next: false,
            next: None,
            countdown: None,
        }
    }

    fn next_episode() -> Launch {
        let mut next = launch(db::TrackPrefs::default(), &[]);
        next.item.title = "Severance".into();
        next.video.season = Some(2);
        next.video.episode = Some(5);
        next
    }

    #[test]
    fn next_up_counts_down_in_state() {
        let mut s = test_session(false);
        s.next = Some(next_episode());
        s.countdown = Some(Countdown { left: 10, tick: tokio::time::Instant::now() });
        assert!(s.count_down().is_empty(), "no on-screen text when the app draws the countdown");
        let up = s.state.lock().unwrap().next_up.clone();
        assert_eq!(up, Some(NextUp { title: "Severance · S02E05".into(), seconds_left: 10 }));
        s.count_down();
        assert_eq!(s.state.lock().unwrap().next_up.as_ref().unwrap().seconds_left, 9);
        let replies = s.handle(&json!({ "event": "client-message", "args": ["cardamoi-cancel"] }));
        assert!(replies.is_empty());
        assert_eq!(s.state.lock().unwrap().next_up, None);
        assert!(s.countdown.is_none() && s.next.is_none());
    }

    #[test]
    fn window_modes_still_count_down_in_mpv() {
        let mut s = test_session(true);
        s.next = Some(next_episode());
        s.countdown = Some(Countdown { left: 3, tick: tokio::time::Instant::now() });
        let replies = s.count_down();
        assert_eq!(replies[0]["command"][0], "show-text");
        assert_eq!(s.state.lock().unwrap().next_up, None);
    }

    #[test]
    fn embedded_resume_goes_to_state_not_osd() {
        let l = launch(db::TrackPrefs::default(), &[]);
        assert!(after_load_commands(&l, false).is_empty());
        assert_eq!(after_load_commands(&l, true)[0]["command"][0], "show-text");
        assert_eq!(initial_state(&l, "Dune".into(), Mode::Embedded).resumed_from, Some(2462.0));
        assert_eq!(initial_state(&l, "Dune".into(), Mode::Window).resumed_from, None);
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
