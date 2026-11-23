//! Android entry point glue.
//!
//! winit needs the [`AndroidApp`] that `android-activity` hands to the
//! `android_main` symbol before it can build an event loop. Store it here from
//! `android_main`, then call the usual `iced::application(..).run()`.
//!
//! Spike only (issue #131): a process-wide slot keeps the iced API unchanged;
//! a real API would take the app through `Settings` or a builder method.
pub use winit::platform::android::activity::AndroidApp;

use std::sync::OnceLock;

static APP: OnceLock<AndroidApp> = OnceLock::new();

/// Stores the [`AndroidApp`] the next `run` uses. Call it first in `android_main`.
pub fn set_app(app: AndroidApp) {
    let _ = APP.set(app);
}

pub(crate) fn app() -> AndroidApp {
    APP.get()
        .cloned()
        .expect("call iced_winit::android::set_app from android_main before running")
}
