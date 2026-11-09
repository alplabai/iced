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

/// Subscribes to failures to build the native menu bar given by
/// `Application::menu`.
///
/// Each item is the reason the build failed, ready to show to the user.
///
/// Only macOS has a native menu bar, so this never fires elsewhere. It fires
/// once for each distinct menu that fails to build, not on every update; a
/// menu taken down and given again is built, and reported, again. The
/// previous menu, if any, stays installed; the application typically falls
/// back to an in-window menu, most needed when the first menu failed and
/// none is installed.
///
/// Nothing is reported when a later, different menu builds successfully
/// (that happens on the next update that changes the rows), so the
/// application can treat a failure as sticky and keep its fallback.
pub fn menu_failures() -> Subscription<String> {
    subscription::from_recipe(MenuFailures)
}

struct MenuFailures;

fn menu_failure(event: subscription::Event) -> Option<String> {
    match event {
        subscription::Event::PlatformSpecific(subscription::PlatformSpecific::MacOS(
            subscription::MacOS::MenuFailed(reason),
        )) => Some(reason),
        _ => None,
    }
}

impl subscription::Recipe for MenuFailures {
    type Output = String;

    fn hash(&self, state: &mut subscription::Hasher) {
        use std::hash::Hash;

        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(self: Box<Self>, input: subscription::EventStream) -> BoxStream<String> {
        use crate::futures::futures::{future, stream::StreamExt};

        boxed_stream(input.filter_map(|event| future::ready(menu_failure(event))))
    }
}

/// Cancels a pending quit request (see [`quit_requests`]); the application
/// keeps running. Does nothing when no request is pending.
pub fn cancel_quit<T>() -> Task<T> {
    task::effect(crate::Action::System(Action::CancelQuit))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mac(event: subscription::MacOS) -> subscription::Event {
        subscription::Event::PlatformSpecific(subscription::PlatformSpecific::MacOS(event))
    }

    #[test]
    fn menu_failure_filter_accepts_only_menu_failed() {
        let failed = subscription::MacOS::MenuFailed("boom".to_owned());

        assert_eq!(menu_failure(mac(failed)), Some("boom".to_owned()));
        assert_eq!(menu_failure(mac(subscription::MacOS::QuitRequested)), None);
        assert_eq!(
            menu_failure(mac(subscription::MacOS::ReceivedUrl(String::new()))),
            None
        );
    }

    /// The shell broadcasts before the runtime polls a new stream, and the
    /// first-install failure is the one an app most needs to see.
    #[test]
    fn a_failure_broadcast_before_the_stream_is_polled_is_delivered() {
        use crate::futures::futures::{StreamExt, channel::mpsc, task::noop_waker};

        use std::task::{Context, Poll};

        let mut tracker = subscription::Tracker::new();
        let (sender, mut received) = mpsc::channel(8);

        let mut futures = tracker.update(
            subscription::into_recipes(menu_failures()).into_iter(),
            sender,
        );
        tracker.broadcast(mac(subscription::MacOS::MenuFailed("boom".to_owned())));

        let waker = noop_waker();
        let mut context = Context::from_waker(&waker);

        // The driver forwards what it buffered, then waits for more.
        let mut driver = futures.remove(0);
        assert!(driver.as_mut().poll(&mut context).is_pending());

        assert_eq!(
            received.poll_next_unpin(&mut context),
            Poll::Ready(Some("boom".to_owned()))
        );
    }

    #[test]
    fn quit_listener_counts_only_while_alive() {
        // The counter is process-wide; other tests do not create listeners.
        let before = QUIT_LISTENERS.load(atomic::Ordering::SeqCst);

        let first = QuitListener::new();
        let second = QuitListener::new();
        assert!(has_quit_listener());
        assert_eq!(QUIT_LISTENERS.load(atomic::Ordering::SeqCst), before + 2);

        drop(first);
        drop(second);
        assert_eq!(QUIT_LISTENERS.load(atomic::Ordering::SeqCst), before);
    }
}
