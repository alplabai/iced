//! Access the native system.
use crate::core::theme;
use crate::futures::futures::channel::oneshot;
use crate::futures::subscription::{self, Subscription};
use crate::futures::{BoxStream, boxed_stream};
use crate::task::{self, Task};

use std::sync::atomic;

/// An operation to be performed on the system.
#[derive(Debug)]
pub enum Action {
    /// Send available system information.
    GetInformation(oneshot::Sender<Information>),

    /// Send the current system theme mode.
    GetTheme(oneshot::Sender<theme::Mode>),

    /// Notify to the runtime that the system theme has changed.
    NotifyTheme(theme::Mode),

    /// Cancel a pending quit request.
    CancelQuit,
}

/// Contains information about the system (e.g. system name, processor, memory, graphics adapter).
#[derive(Clone, Debug)]
pub struct Information {
    /// The operating system name
    pub system_name: Option<String>,
    /// Operating system kernel version
    pub system_kernel: Option<String>,
    /// Long operating system version
    ///
    /// Examples:
    /// - MacOS 10.15 Catalina
    /// - Windows 10 Pro
    /// - Ubuntu 20.04 LTS (Focal Fossa)
    pub system_version: Option<String>,
    /// Short operating system version number
    pub system_short_version: Option<String>,
    /// Detailed processor model information
    pub cpu_brand: String,
    /// The number of physical cores on the processor
    pub cpu_cores: Option<usize>,
    /// Total RAM size, in bytes
    pub memory_total: u64,
    /// Memory used by this process, in bytes
    pub memory_used: Option<u64>,
    /// Underlying graphics backend for rendering
    pub graphics_backend: String,
    /// Model information for the active graphics adapter
    pub graphics_adapter: String,
}

/// Returns available system information.
pub fn information() -> Task<Information> {
    task::oneshot(|channel| crate::Action::System(Action::GetInformation(channel)))
}

/// Returns the current system theme.
pub fn theme() -> Task<theme::Mode> {
    task::oneshot(|sender| crate::Action::System(Action::GetTheme(sender)))
}

/// Subscribes to system theme changes.
pub fn theme_changes() -> Subscription<theme::Mode> {
    #[derive(Hash)]
    struct ThemeChanges;

    subscription::filter_map(ThemeChanges, |event| {
        let subscription::Event::SystemThemeChanged(mode) = event else {
            return None;
        };

        Some(mode)
    })
}

/// Subscribes to requests to quit the application.
///
/// On macOS a quit request is Command+Q, Dock ▸ Quit, logout or
/// [`Predefined::Quit`](crate::core::menu::Predefined::Quit). It only reaches
/// the application when `Settings::exit_on_quit_request` is `false`; the
/// application must then answer every request, with [`exit`](crate::exit) to
/// quit or [`cancel_quit`] to stay. While it has not answered, macOS waits;
/// at logout it only waits a limited time before it reports that the
/// application is blocking logout.
///
/// A request that arrives while no [`quit_requests`] subscription is running
/// is answered at once with "quit", as if `exit_on_quit_request` were `true`,
/// so a missing or conditional subscription cannot leave a quit unanswered.
///
/// Other platforms have no application-wide quit request; there, closing
/// windows goes through `window::close_requests`.
pub fn quit_requests() -> Subscription<()> {
    subscription::from_recipe(QuitRequests)
}

/// Returns whether a [`quit_requests`] subscription is running.
///
/// The shell uses it to answer a quit request nobody would see.
#[doc(hidden)]
pub fn has_quit_listener() -> bool {
    QUIT_LISTENERS.load(atomic::Ordering::SeqCst) > 0
}

static QUIT_LISTENERS: atomic::AtomicUsize = atomic::AtomicUsize::new(0);

struct QuitRequests;

/// Counts a running [`quit_requests`] stream for as long as it lives.
struct QuitListener;

impl QuitListener {
    fn new() -> Self {
        let _ = QUIT_LISTENERS.fetch_add(1, atomic::Ordering::SeqCst);
        Self
    }
}

impl Drop for QuitListener {
    fn drop(&mut self) {
        let _ = QUIT_LISTENERS.fetch_sub(1, atomic::Ordering::SeqCst);
    }
}

impl subscription::Recipe for QuitRequests {
    type Output = ();

    fn hash(&self, state: &mut subscription::Hasher) {
        use std::hash::Hash;

        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(self: Box<Self>, input: subscription::EventStream) -> BoxStream<()> {
        use crate::futures::futures::{future, stream::StreamExt};

        // Created here, not in `quit_requests`, so only a stream the runtime
        // actually runs is counted.
        let listener = QuitListener::new();

        boxed_stream(input.filter_map(move |event| {
            let _ = &listener;

            future::ready(match event {
                subscription::Event::PlatformSpecific(subscription::PlatformSpecific::MacOS(
                    subscription::MacOS::QuitRequested,
                )) => Some(()),
                _ => None,
            })
        }))
    }
}

/// Cancels a pending quit request (see [`quit_requests`]); the application
/// keeps running. Does nothing when no request is pending.
pub fn cancel_quit<T>() -> Task<T> {
    task::effect(crate::Action::System(Action::CancelQuit))
}
