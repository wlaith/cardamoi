//! The native view the built-in player draws into. It sits under the web view,
//! which is transparent while a video plays, so the player's controls are the
//! app's own UI over the picture.

/// Whether this platform can draw the picture inside the app window.
pub const EMBEDS: bool = cfg!(target_os = "macos");

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::Surface;

#[cfg(not(target_os = "macos"))]
mod none;
#[cfg(not(target_os = "macos"))]
pub use none::Surface;
