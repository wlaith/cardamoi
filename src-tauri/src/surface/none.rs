use tauri::{AppHandle, WebviewWindow};

use crate::libmpv::Mpv;

/// Platforms that don't embed play in mpv's own window.
pub struct Surface;

impl Surface {
    pub fn attach(_window: &WebviewWindow) -> anyhow::Result<Surface> {
        anyhow::bail!("this platform doesn't embed the player")
    }

    pub fn connect(&self, _mpv: &mut Mpv) -> anyhow::Result<()> {
        anyhow::bail!("this platform doesn't embed the player")
    }

    pub fn set_visible(&self, _app: &AppHandle, _visible: bool) {}
}
