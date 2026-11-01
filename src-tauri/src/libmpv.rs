//! A bundled libmpv, loaded at runtime so that a missing or damaged library is
//! an error the app can handle instead of a crash at launch.

use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

use anyhow::{Context, anyhow, bail};
use libloading::Library;
use tokio::sync::oneshot;

type Handle = *mut c_void;

const EVENT_SHUTDOWN: c_int = 1;

// mpv_render_param_type, from mpv/render.h.
const RENDER_API_TYPE: c_int = 1;
const RENDER_OPENGL_INIT_PARAMS: c_int = 2;
const RENDER_OPENGL_FBO: c_int = 3;
const RENDER_FLIP_Y: c_int = 4;

/// Looks up an OpenGL function by name, for libmpv's renderer.
pub type GetProc = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;

/// Runs a closure with the renderer's OpenGL context current: libmpv frees its GL objects when the renderer goes.
pub type WithGl = Box<dyn Fn(&mut dyn FnMut()) + Send + Sync>;

#[repr(C)]
struct RenderParam {
    kind: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct GlInitParams {
    get_proc_address: Option<GetProc>,
    get_proc_address_ctx: *mut c_void,
}

#[repr(C)]
struct GlFbo {
    fbo: c_int,
    w: c_int,
    h: c_int,
    internal_format: c_int,
}

#[repr(C)]
struct Event {
    event_id: c_int,
}

struct Api {
    create: unsafe extern "C" fn() -> Handle,
    set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    initialize: unsafe extern "C" fn(Handle) -> c_int,
    #[cfg_attr(not(test), allow(dead_code))]
    command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    wait_event: unsafe extern "C" fn(Handle, f64) -> *const Event,
    wakeup: unsafe extern "C" fn(Handle),
    terminate_destroy: unsafe extern "C" fn(Handle),
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    render_create: unsafe extern "C" fn(*mut Handle, Handle, *mut RenderParam) -> c_int,
    render_set_update_callback: unsafe extern "C" fn(Handle, Option<unsafe extern "C" fn(*mut c_void)>, *mut c_void),
    render_update: unsafe extern "C" fn(Handle) -> u64,
    render_render: unsafe extern "C" fn(Handle, *mut RenderParam) -> c_int,
    render_report_swap: unsafe extern "C" fn(Handle),
    render_free: unsafe extern "C" fn(Handle),
    // Last, so the function pointers above never outlive the library.
    _lib: Library,
}

impl Api {
    fn open(path: &Path) -> anyhow::Result<Api> {
        // SAFETY: loading a shared library runs its initialisers; ours is libmpv.
        let lib = unsafe { Library::new(path) }.with_context(|| format!("couldn't load {}", path.display()))?;
        macro_rules! sym {
            ($name:literal) => {
                // SAFETY: the signature matches libmpv's client.h.
                *unsafe { lib.get($name) }.with_context(|| format!("{} has no {}", path.display(), String::from_utf8_lossy($name)))?
            };
        }
        Ok(Api {
            create: sym!(b"mpv_create"),
            set_option_string: sym!(b"mpv_set_option_string"),
            initialize: sym!(b"mpv_initialize"),
            command: sym!(b"mpv_command"),
            wait_event: sym!(b"mpv_wait_event"),
            wakeup: sym!(b"mpv_wakeup"),
            terminate_destroy: sym!(b"mpv_terminate_destroy"),
            error_string: sym!(b"mpv_error_string"),
            render_create: sym!(b"mpv_render_context_create"),
            render_set_update_callback: sym!(b"mpv_render_context_set_update_callback"),
            render_update: sym!(b"mpv_render_context_update"),
            render_render: sym!(b"mpv_render_context_render"),
            render_report_swap: sym!(b"mpv_render_context_report_swap"),
            render_free: sym!(b"mpv_render_context_free"),
            _lib: lib,
        })
    }

    fn check(&self, code: c_int, what: &str) -> anyhow::Result<()> {
        if code >= 0 {
            return Ok(());
        }
        // SAFETY: mpv_error_string returns a static string.
        let text = unsafe { CStr::from_ptr((self.error_string)(code)) }.to_string_lossy();
        bail!("{what}: {text}")
    }
}

/// Where the library is looked for: the one shipped with the app first, then
/// (on Linux) the system's.
pub fn candidates(resource_dir: &Path) -> Vec<PathBuf> {
    let bundled = resource_dir.join("mpv");
    let mut found = if cfg!(target_os = "macos") {
        vec![bundled.join("libmpv.2.dylib")]
    } else if cfg!(windows) {
        vec![bundled.join("libmpv-2.dll")]
    } else {
        vec![bundled.join("libmpv.so.2"), bundled.join("libmpv.so.1")]
    };
    if cfg!(all(unix, not(target_os = "macos"))) {
        found.extend(["libmpv.so.2", "libmpv.so.1"].map(PathBuf::from));
    }
    found
}

/// Whether some candidate loads.
pub fn available(resource_dir: &Path) -> bool {
    let mut why = Vec::new();
    for path in candidates(resource_dir) {
        match Api::open(&path) {
            Ok(_) => return true,
            Err(e) => why.push(format!("{e:#}")),
        }
    }
    eprintln!("libmpv isn't usable: {}", why.join("; "));
    false
}

struct Ptr(Handle);
// SAFETY: libmpv's client API is thread safe, apart from waiting for events,
// which only the watcher thread does.
unsafe impl Send for Ptr {}
unsafe impl Sync for Ptr {}

/// libmpv's renderer: it draws video frames into a framebuffer the app owns.
pub struct Render {
    api: Arc<Api>,
    ctx: Mutex<Option<Ptr>>,
    on_update: Box<dyn Fn() + Send + Sync>,
    with_gl: WithGl,
}

extern "C" fn render_updated(data: *mut c_void) {
    // SAFETY: `data` is the `Render` that registered this callback, kept alive until `free` clears it.
    let render = unsafe { &*(data as *const Render) };
    (render.on_update)();
}

impl Render {
    /// Draws the current frame into `fbo`, `width` by `height` pixels. False when the player is gone.
    pub fn draw(&self, fbo: i32, width: i32, height: i32) -> bool {
        let guard = self.ctx.lock().unwrap();
        let Some(ctx) = &*guard else { return false };
        let mut target = GlFbo { fbo, w: width, h: height, internal_format: 0 };
        let mut flip: c_int = 1;
        let mut params = [
            RenderParam { kind: RENDER_OPENGL_FBO, data: (&raw mut target).cast() },
            RenderParam { kind: RENDER_FLIP_Y, data: (&raw mut flip).cast() },
            RenderParam { kind: 0, data: std::ptr::null_mut() },
        ];
        // SAFETY: the context is alive while it is in the slot; the params outlive the call.
        unsafe {
            // mpv wants to hear that the update was taken before it is drawn.
            (self.api.render_update)(ctx.0);
            (self.api.render_render)(ctx.0, params.as_mut_ptr());
            (self.api.render_report_swap)(ctx.0);
        }
        true
    }

    fn free(&self) {
        (self.with_gl)(&mut || {
            if let Some(ctx) = self.ctx.lock().unwrap().take() {
                // SAFETY: valid context; the callback is removed before it is freed, with the GL context current.
                unsafe {
                    (self.api.render_set_update_callback)(ctx.0, None, std::ptr::null_mut());
                    (self.api.render_free)(ctx.0);
                }
            }
        });
    }
}

pub struct Mpv {
    api: Arc<Api>,
    handle: Arc<Ptr>,
    stop: Arc<AtomicBool>,
    watcher: Option<JoinHandle<()>>,
    render: Option<Arc<Render>>,
}

impl Mpv {
    /// Creates a handle on the first candidate that loads; it isn't initialised.
    pub fn load(candidates: &[PathBuf]) -> anyhow::Result<Mpv> {
        #[cfg(unix)]
        // libmpv wants the C numeric locale, which GUI toolkits may have changed.
        unsafe {
            unsafe extern "C" {
                fn setlocale(category: c_int, locale: *const c_char) -> *const c_char;
            }
            const LC_NUMERIC: c_int = if cfg!(target_os = "macos") { 4 } else { 1 };
            setlocale(LC_NUMERIC, c"C".as_ptr());
        }
        let mut last = anyhow!("no libmpv to try");
        for path in candidates {
            match Api::open(path) {
                Ok(api) => {
                    // SAFETY: mpv_create takes no arguments.
                    let handle = unsafe { (api.create)() };
                    if handle.is_null() {
                        last = anyhow!("mpv_create failed for {}", path.display());
                        continue;
                    }
                    return Ok(Mpv {
                        api: Arc::new(api),
                        handle: Arc::new(Ptr(handle)),
                        stop: Arc::new(AtomicBool::new(false)),
                        watcher: None,
                        render: None,
                    });
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    pub fn set_option(&self, name: &str, value: &str) -> anyhow::Result<()> {
        let (n, v) = (CString::new(name)?, CString::new(value)?);
        // SAFETY: valid handle and NUL-terminated strings.
        let code = unsafe { (self.api.set_option_string)(self.handle.0, n.as_ptr(), v.as_ptr()) };
        self.api.check(code, &format!("option {name}={value}"))
    }

    pub fn initialize(&self) -> anyhow::Result<()> {
        // SAFETY: valid handle.
        self.api.check(unsafe { (self.api.initialize)(self.handle.0) }, "mpv_initialize")
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn command(&self, args: &[&str]) -> anyhow::Result<()> {
        let owned: Vec<CString> = args.iter().map(|a| CString::new(*a)).collect::<Result<_, _>>()?;
        let mut argv: Vec<*const c_char> = owned.iter().map(|a| a.as_ptr()).collect();
        argv.push(std::ptr::null());
        // SAFETY: argv is NULL-terminated and outlives the call.
        let code = unsafe { (self.api.command)(self.handle.0, argv.as_mut_ptr()) };
        self.api.check(code, &format!("command {}", args.first().copied().unwrap_or_default()))
    }

    /// Starts the OpenGL renderer (set `vo=libmpv` before `initialize`). An OpenGL context must be current.
    /// `on_update` runs on an arbitrary thread whenever a new frame should be drawn, and must not call mpv.
    /// `with_gl` runs a closure with that context current, for when the renderer is freed.
    pub fn create_render(
        &mut self,
        get_proc: GetProc,
        on_update: impl Fn() + Send + Sync + 'static,
        with_gl: WithGl,
    ) -> anyhow::Result<Arc<Render>> {
        let mut init = GlInitParams { get_proc_address: Some(get_proc), get_proc_address_ctx: std::ptr::null_mut() };
        let mut params = [
            RenderParam { kind: RENDER_API_TYPE, data: c"opengl".as_ptr().cast_mut().cast() },
            RenderParam { kind: RENDER_OPENGL_INIT_PARAMS, data: (&raw mut init).cast() },
            RenderParam { kind: 0, data: std::ptr::null_mut() },
        ];
        let mut ctx: Handle = std::ptr::null_mut();
        // SAFETY: valid handle; the params outlive the call.
        let code = unsafe { (self.api.render_create)(&mut ctx, self.handle.0, params.as_mut_ptr()) };
        self.api.check(code, "mpv_render_context_create")?;
        let render = Arc::new(Render { api: self.api.clone(), ctx: Mutex::new(Some(Ptr(ctx))), on_update: Box::new(on_update), with_gl });
        // SAFETY: the callback data stays valid until `Render::free`, which runs before the handle is destroyed.
        unsafe { (self.api.render_set_update_callback)(ctx, Some(render_updated), Arc::as_ptr(&render) as *mut c_void) };
        self.render = Some(render.clone());
        Ok(render)
    }

    /// Resolves when mpv shuts down (its window closed, or it quit). Call once, after `initialize`.
    pub fn watch_shutdown(&mut self) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let (api, handle, stop) = (self.api.clone(), self.handle.clone(), self.stop.clone());
        self.watcher = Some(std::thread::spawn(move || {
            loop {
                // SAFETY: valid handle; this thread is the only one that waits for events.
                let event = unsafe { (api.wait_event)(handle.0, -1.0) };
                if stop.load(Ordering::Acquire) {
                    return;
                }
                if !event.is_null() && unsafe { (*event).event_id } == EVENT_SHUTDOWN {
                    let _ = tx.send(());
                    return;
                }
            }
        }));
        rx
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // SAFETY: valid handle; the watcher is joined before it is destroyed.
        unsafe { (self.api.wakeup)(self.handle.0) };
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
        // The renderer goes first, then the player it draws for.
        if let Some(render) = self.render.take() {
            render.free();
        }
        unsafe { (self.api.terminate_destroy)(self.handle.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        net::TcpListener,
        time::{Duration, Instant},
    };

    fn test_candidates() -> Vec<PathBuf> {
        let vendor = Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor");
        let mut c = candidates(&vendor);
        c.push(PathBuf::from("/opt/homebrew/lib/libmpv.2.dylib"));
        c
    }

    #[test]
    fn bundled_library_comes_first() {
        let c = candidates(Path::new("/app/res"));
        assert!(c[0].starts_with("/app/res/mpv"));
    }

    #[test]
    fn missing_library_is_an_error_not_a_crash() {
        assert!(Mpv::load(&[PathBuf::from("/nowhere/libmpv.dylib")]).is_err());
    }

    #[test]
    #[ignore = "needs libmpv; run in CI after fetch-libmpv"]
    fn opens_and_shuts_down() {
        let mut mpv = Mpv::load(&test_candidates()).unwrap();
        mpv.set_option("vo", "null").unwrap();
        mpv.set_option("idle", "yes").unwrap();
        mpv.initialize().unwrap();
        let _shutdown = mpv.watch_shutdown();
        let start = Instant::now();
        drop(mpv);
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    #[ignore = "needs libmpv"]
    fn stop_while_opening_destroys_handle() {
        // A server that accepts and never answers keeps mpv stuck in "opening".
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/film.mkv", server.local_addr().unwrap());
        let mut mpv = Mpv::load(&test_candidates()).unwrap();
        mpv.set_option("vo", "null").unwrap();
        mpv.set_option("ao", "null").unwrap();
        mpv.initialize().unwrap();
        let _shutdown = mpv.watch_shutdown();
        mpv.command(&["loadfile", &url]).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let start = Instant::now();
        drop(mpv);
        assert!(start.elapsed() < Duration::from_secs(2), "took {:?}", start.elapsed());
    }

    #[test]
    #[ignore = "needs libmpv"]
    fn bad_option_is_reported() {
        let mpv = Mpv::load(&test_candidates()).unwrap();
        let err = mpv.set_option("no-such-option", "1").unwrap_err();
        assert!(err.to_string().contains("no-such-option"));
    }
}
