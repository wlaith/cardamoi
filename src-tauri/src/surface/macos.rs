//! The video view on macOS. mpv 0.41 can't draw into a view it is handed, so
//! libmpv's render API draws into a `CAOpenGLLayer` of our own, in a view
//! beneath the web view.

use std::{
    ffi::{c_char, c_int, c_void},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
    },
};

use objc2::{
    msg_send, sel,
    runtime::{AnyClass, AnyObject, ClassBuilder, Bool, Sel},
};
use objc2_foundation::NSRect;
use tauri::{AppHandle, WebviewWindow};

use crate::libmpv::{Mpv, Render};

const WIDTH_SIZABLE: usize = 2;
const HEIGHT_SIZABLE: usize = 16;
const WINDOW_BELOW: isize = -1;

const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
const GL_VIEWPORT: u32 = 0x0BA2;
const GL_COLOR_BUFFER_BIT: u32 = 0x4000;

#[link(name = "OpenGL", kind = "framework")]
unsafe extern "C" {
    fn glGetIntegerv(name: u32, data: *mut i32);
    fn glClearColor(r: f32, g: f32, b: f32, a: f32);
    fn glClear(mask: u32);
    fn CGLChoosePixelFormat(attributes: *const i32, pixel_format: *mut *mut c_void, count: *mut i32) -> i32;
    fn CGLCreateContext(pixel_format: *mut c_void, share: *mut c_void, context: *mut *mut c_void) -> i32;
    fn CGLRetainContext(context: *mut c_void) -> *mut c_void;
    fn CGLRetainPixelFormat(pixel_format: *mut c_void) -> *mut c_void;
    fn CGLSetCurrentContext(context: *mut c_void) -> i32;
    fn CGLLockContext(context: *mut c_void) -> i32;
    fn CGLUnlockContext(context: *mut c_void) -> i32;
}

#[link(name = "QuartzCore", kind = "framework")]
unsafe extern "C" {}

unsafe extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

/// OpenGL function lookup for libmpv: the framework is linked, so the default namespace has them.
unsafe extern "C" fn get_proc_address(_: *mut c_void, name: *const c_char) -> *mut c_void {
    const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
    unsafe { dlsym(RTLD_DEFAULT, name) }
}

/// What the layer's callbacks share with the rest of the app; there is one video view per process.
struct Shared {
    context: AtomicUsize,
    pixel_format: AtomicUsize,
    render: Mutex<Option<Arc<Render>>>,
    frame_ready: AtomicBool,
    last_size: AtomicI64,
}

static SHARED: OnceLock<Shared> = OnceLock::new();

fn shared() -> &'static Shared {
    SHARED.get_or_init(|| Shared {
        context: AtomicUsize::new(0),
        pixel_format: AtomicUsize::new(0),
        render: Mutex::new(None),
        frame_ready: AtomicBool::new(true),
        last_size: AtomicI64::new(0),
    })
}

// The layer's overrides. Pixel formats and contexts are passed around as opaque pointers.

unsafe extern "C-unwind" fn copy_pixel_format(_: *mut AnyObject, _: Sel, _display_mask: u32) -> *mut c_void {
    unsafe { CGLRetainPixelFormat(shared().pixel_format.load(Ordering::Acquire) as *mut c_void) }
}

unsafe extern "C-unwind" fn copy_context(_: *mut AnyObject, _: Sel, _pixel_format: *mut c_void) -> *mut c_void {
    unsafe { CGLRetainContext(shared().context.load(Ordering::Acquire) as *mut c_void) }
}

unsafe extern "C-unwind" fn can_draw(_: *mut AnyObject, _: Sel, _: *mut c_void, _: *mut c_void, _: f64, _: *const c_void) -> Bool {
    Bool::new(shared().frame_ready.load(Ordering::Acquire))
}

unsafe extern "C-unwind" fn draw(_: *mut AnyObject, _: Sel, _: *mut c_void, _: *mut c_void, _: f64, _: *const c_void) {
    let shared = shared();
    let (mut fbo, mut viewport) = (0, [0i32; 4]);
    // SAFETY: Core Animation made the layer's context current and bound its framebuffer.
    unsafe {
        glGetIntegerv(GL_FRAMEBUFFER_BINDING, &mut fbo);
        glGetIntegerv(GL_VIEWPORT, viewport.as_mut_ptr());
    }
    let (width, height) = (viewport[2], viewport[3]);
    shared.last_size.store(((width as i64) << 32) | height as i64, Ordering::Release);
    shared.frame_ready.store(false, Ordering::Release);
    let render = shared.render.lock().unwrap().clone();
    let drawn = render.is_some_and(|r| r.draw(fbo, width, height));
    if !drawn {
        // SAFETY: as above.
        unsafe {
            glClearColor(0.0, 0.0, 0.0, 1.0);
            glClear(GL_COLOR_BUFFER_BIT);
        }
    }
}

fn layer_class() -> &'static AnyClass {
    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLASS.get_or_init(|| {
        let superclass = AnyClass::get(c"CAOpenGLLayer").expect("QuartzCore is linked");
        let mut builder = ClassBuilder::new(c"CardamoiVideoLayer", superclass).expect("the class name is free");
        // SAFETY: the signatures match CAOpenGLLayer's documented overrides.
        unsafe {
            builder.add_method(
                sel!(copyCGLPixelFormatForDisplayMask:),
                copy_pixel_format as unsafe extern "C-unwind" fn(*mut AnyObject, Sel, u32) -> *mut c_void,
            );
            builder.add_method(
                sel!(copyCGLContextForPixelFormat:),
                copy_context as unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut c_void) -> *mut c_void,
            );
            builder.add_method(
                sel!(canDrawInCGLContext:pixelFormat:forLayerTime:displayTime:),
                can_draw as unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut c_void, *mut c_void, f64, *const c_void) -> Bool,
            );
            builder.add_method(
                sel!(drawInCGLContext:pixelFormat:forLayerTime:displayTime:),
                draw as unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut c_void, *mut c_void, f64, *const c_void),
            );
        }
        builder.register()
    })
}

/// An `NSView` filling the window, beneath the web view, whose layer shows libmpv's frames.
pub struct Surface {
    view: Arc<AtomicUsize>,
}

impl Surface {
    /// Adds the view; hidden until `set_visible`. Call from the main thread.
    pub fn attach(window: &WebviewWindow) -> anyhow::Result<Surface> {
        create_gl_context()?;
        let view = Arc::new(AtomicUsize::new(0));
        let slot = view.clone();
        window.with_webview(move |webview| {
            // SAFETY: the pointers come from Tauri and are used on the main thread.
            unsafe {
                let web: *mut AnyObject = webview.inner().cast();
                let parent: *mut AnyObject = msg_send![web, superview];
                let frame: NSRect = msg_send![web, frame];
                let made: *mut AnyObject = msg_send![objc2::class!(NSView), alloc];
                let made: *mut AnyObject = msg_send![made, initWithFrame: frame];
                let layer: *mut AnyObject = msg_send![layer_class(), new];
                let window: *mut AnyObject = msg_send![web, window];
                let scale: f64 = msg_send![window, backingScaleFactor];
                let _: () = msg_send![layer, setContentsScale: scale];
                let _: () = msg_send![layer, setAsynchronous: Bool::YES];
                let _: () = msg_send![layer, setOpaque: Bool::YES];
                // A layer-hosting view: the layer is set before the view asks for one.
                let _: () = msg_send![made, setLayer: layer];
                let _: () = msg_send![made, setWantsLayer: Bool::YES];
                let _: () = msg_send![made, setAutoresizingMask: WIDTH_SIZABLE | HEIGHT_SIZABLE];
                let _: () = msg_send![made, setHidden: Bool::YES];
                let _: () = msg_send![parent, addSubview: made, positioned: WINDOW_BELOW, relativeTo: web];
                slot.store(made as usize, Ordering::Release);
            }
        })?;
        Ok(Surface { view })
    }

    /// Points the layer at `mpv`'s renderer. `mpv` must have `vo=libmpv` and be initialised.
    pub fn connect(&self, mpv: &mut Mpv) -> anyhow::Result<()> {
        let mut made = None;
        with_gl_current(&mut || {
            made = Some(mpv.create_render(
                get_proc_address,
                || shared().frame_ready.store(true, Ordering::Release),
                Box::new(with_gl_current),
            ));
        });
        let render = made.expect("the closure ran")?;
        *shared().render.lock().unwrap() = Some(render);
        shared().frame_ready.store(true, Ordering::Release);
        Ok(())
    }

    pub fn set_visible(&self, app: &AppHandle, visible: bool) {
        let view = self.view.clone();
        let _ = app.run_on_main_thread(move || {
            let view = view.load(Ordering::Acquire) as *mut AnyObject;
            if !view.is_null() {
                // SAFETY: the view is retained by its superview for the life of the window.
                unsafe {
                    let _: () = msg_send![view, setHidden: Bool::new(!visible)];
                }
                shared().frame_ready.store(true, Ordering::Release);
            }
        });
    }
}

/// Runs `f` with the layer's OpenGL context current on this thread (the context isn't thread safe).
fn with_gl_current(f: &mut dyn FnMut()) {
    let context = shared().context.load(Ordering::Acquire) as *mut c_void;
    // SAFETY: the context was made by `create_gl_context` and stays alive for the process.
    unsafe {
        CGLLockContext(context);
        CGLSetCurrentContext(context);
    }
    f();
    // SAFETY: as above.
    unsafe {
        CGLSetCurrentContext(std::ptr::null_mut());
        CGLUnlockContext(context);
    }
}

/// The OpenGL context the layer draws with, made once. Core 3.2 is what libmpv's renderer prefers.
fn create_gl_context() -> anyhow::Result<()> {
    let shared = shared();
    if shared.context.load(Ordering::Acquire) != 0 {
        return Ok(());
    }
    const ACCELERATED: i32 = 73;
    const DOUBLE_BUFFER: i32 = 5;
    const OPENGL_PROFILE: i32 = 99;
    const PROFILE_3_2_CORE: i32 = 0x3200;
    const ALLOW_OFFLINE_RENDERERS: i32 = 96;
    let attributes = [ACCELERATED, DOUBLE_BUFFER, ALLOW_OFFLINE_RENDERERS, OPENGL_PROFILE, PROFILE_3_2_CORE, 0];
    let (mut format, mut count, mut context): (*mut c_void, c_int, *mut c_void) = (std::ptr::null_mut(), 0, std::ptr::null_mut());
    // SAFETY: the attribute list is zero-terminated.
    unsafe {
        let code = CGLChoosePixelFormat(attributes.as_ptr(), &mut format, &mut count);
        anyhow::ensure!(code == 0 && !format.is_null(), "no OpenGL pixel format (error {code})");
        let code = CGLCreateContext(format, std::ptr::null_mut(), &mut context);
        anyhow::ensure!(code == 0 && !context.is_null(), "couldn't create an OpenGL context (error {code})");
    }
    shared.pixel_format.store(format as usize, Ordering::Release);
    shared.context.store(context as usize, Ordering::Release);
    Ok(())
}
