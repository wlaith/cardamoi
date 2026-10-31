use std::sync::Mutex;

use tauri::{App, AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_deep_link::DeepLinkExt;

/// The last magnet link the OS handed us, kept until the window picks it up.
/// The app can be launched by a click before the page is listening.
#[derive(Default)]
pub struct PendingMagnet(Mutex<Option<String>>);

pub fn setup(app: &App) {
    app.manage(PendingMagnet::default());
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        receive(app.handle(), urls.iter().map(|u| u.as_str()));
    }
    let handle = app.handle().clone();
    app.deep_link().on_open_url(move |event| {
        let urls = event.urls();
        receive(&handle, urls.iter().map(|u| u.as_str()));
    });
}

fn receive<'a, R: Runtime>(app: &AppHandle<R>, urls: impl Iterator<Item = &'a str>) {
    let Some(magnet) = urls.filter(|u| u.starts_with("magnet:")).last() else { return };
    *app.state::<PendingMagnet>().0.lock().unwrap() = Some(magnet.to_owned());
    focus_main(app);
    let _ = app.emit("open-magnet", ());
}

pub fn focus_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub fn take_pending_magnet(pending: tauri::State<'_, PendingMagnet>) -> Option<String> {
    pending.0.lock().unwrap().take()
}

#[tauri::command]
pub fn magnet_handler_is_us(app: AppHandle) -> Result<bool, String> {
    os::is_default(&app).map_err(|e| format!("{e:#}"))
}

/// Makes cardamoi the default app for magnet links. Returns true when the OS
/// needs the user to finish the change in its own settings window.
#[tauri::command]
pub fn make_magnet_handler(app: AppHandle) -> Result<bool, String> {
    os::make_default(&app).map_err(|e| format!("{e:#}"))
}

#[cfg(target_os = "macos")]
mod os {
    use core_foundation::base::TCFType;
    use core_foundation::string::{CFString, CFStringRef};
    use tauri::AppHandle;

    #[link(name = "CoreServices", kind = "framework")]
    unsafe extern "C" {
        fn LSCopyDefaultHandlerForURLScheme(scheme: CFStringRef) -> CFStringRef;
        fn LSSetDefaultHandlerForURLScheme(scheme: CFStringRef, bundle_id: CFStringRef) -> i32;
    }

    pub fn is_default(app: &AppHandle) -> anyhow::Result<bool> {
        let scheme = CFString::new("magnet");
        let handler = unsafe { LSCopyDefaultHandlerForURLScheme(scheme.as_concrete_TypeRef()) };
        if handler.is_null() {
            return Ok(false);
        }
        let handler = unsafe { CFString::wrap_under_create_rule(handler) }.to_string();
        Ok(handler.eq_ignore_ascii_case(&app.config().identifier))
    }

    pub fn make_default(app: &AppHandle) -> anyhow::Result<bool> {
        let scheme = CFString::new("magnet");
        let id = CFString::new(&app.config().identifier);
        let status = unsafe { LSSetDefaultHandlerForURLScheme(scheme.as_concrete_TypeRef(), id.as_concrete_TypeRef()) };
        anyhow::ensure!(status == 0, "macOS refused to change the default app (error {status})");
        Ok(false)
    }
}

#[cfg(target_os = "linux")]
mod os {
    use tauri::AppHandle;
    use tauri_plugin_deep_link::DeepLinkExt;

    pub fn is_default(app: &AppHandle) -> anyhow::Result<bool> {
        Ok(app.deep_link().is_registered("magnet")?)
    }

    pub fn make_default(app: &AppHandle) -> anyhow::Result<bool> {
        // Runs `xdg-mime default`, which sets the default as well as registering.
        app.deep_link().register("magnet")?;
        Ok(false)
    }
}

#[cfg(windows)]
mod os {
    use std::process::Command;

    use tauri::AppHandle;
    use tauri_plugin_deep_link::DeepLinkExt;
    use tauri_plugin_opener::OpenerExt;

    /// The app Windows was told to use for magnet links in its own settings, if any.
    /// This choice wins over the registration in Software\Classes.
    fn user_choice() -> Option<String> {
        let out = Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Windows\Shell\Associations\UrlAssociations\magnet\UserChoice",
                "/v",
                "ProgId",
            ])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let line = text.lines().find(|l| l.trim_start().starts_with("ProgId"))?;
        line.split_whitespace().last().map(str::to_owned)
    }

    fn overridden() -> bool {
        // Our registration lives under the "magnet" ProgId itself.
        user_choice().is_some_and(|id| !id.eq_ignore_ascii_case("magnet"))
    }

    pub fn is_default(app: &AppHandle) -> anyhow::Result<bool> {
        Ok(app.deep_link().is_registered("magnet")? && !overridden())
    }

    pub fn make_default(app: &AppHandle) -> anyhow::Result<bool> {
        app.deep_link().register("magnet")?;
        if !overridden() {
            return Ok(false);
        }
        // Windows protects the user's choice; only its Settings app can change it.
        app.opener().open_url("ms-settings:defaultapps", None::<&str>)?;
        Ok(true)
    }
}
