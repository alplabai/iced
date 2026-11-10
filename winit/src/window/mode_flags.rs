use crate::core::window;

/// The maximized, minimized and fullscreen flags of a window.
///
/// It remembers the last known flags so that a change can be detected
/// without ever comparing window sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModeFlags {
    maximized: bool,
    minimized: Option<bool>,
    fullscreen: bool,
}

impl ModeFlags {
    /// Creates [`ModeFlags`] from the given flags.
    pub fn new(maximized: bool, minimized: Option<bool>, fullscreen: bool) -> Self {
        Self {
            maximized,
            minimized,
            fullscreen,
        }
    }

    /// Reads the current [`ModeFlags`] of a [`winit::window::Window`].
    ///
    /// `maximized_hint` is used instead of asking the window for windows on
    /// macOS that are undecorated or not resizable, where `is_maximized`
    /// temporarily changes the window's style mask and can feed back into new
    /// `Resized` events.
    pub fn read(window: &winit::window::Window, maximized_hint: bool) -> Self {
        Self::new(
            if Self::reads_maximized(window) {
                window.is_maximized()
            } else {
                maximized_hint
            },
            window.is_minimized(),
            window.fullscreen().is_some(),
        )
    }

    /// Whether `is_maximized` can be called on the window without side effects.
    pub fn reads_maximized(window: &winit::window::Window) -> bool {
        !cfg!(target_os = "macos") || (window.is_decorated() && window.is_resizable())
    }

    /// Whether the window is maximized.
    pub fn maximized(self) -> bool {
        self.maximized
    }

    /// Stores the `current` flags and returns a [`window::Event::ModeChanged`]
    /// if they differ from the previously stored ones.
    pub fn update(&mut self, current: Self) -> Option<window::Event> {
        if *self == current {
            return None;
        }

        *self = current;

        Some(window::Event::ModeChanged {
            maximized: current.maximized,
            minimized: current.minimized,
            fullscreen: current.fullscreen,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changed(
        maximized: bool,
        minimized: Option<bool>,
        fullscreen: bool,
    ) -> Option<window::Event> {
        Some(window::Event::ModeChanged {
            maximized,
            minimized,
            fullscreen,
        })
    }

    #[test]
    fn no_change_emits_nothing() {
        let mut flags = ModeFlags::default();

        assert_eq!(flags.update(ModeFlags::default()), None);

        let mut flags = ModeFlags::new(true, Some(false), false);
        assert_eq!(flags.update(ModeFlags::new(true, Some(false), false)), None);
    }

    #[test]
    fn maximize_and_unmaximize() {
        let mut flags = ModeFlags::default();

        assert_eq!(
            flags.update(ModeFlags::new(true, Some(false), false)),
            changed(true, Some(false), false)
        );
        assert_eq!(
            flags.update(ModeFlags::new(false, Some(false), false)),
            changed(false, Some(false), false)
        );
    }

    #[test]
    fn minimize_and_restore() {
        let mut flags = ModeFlags::new(true, Some(false), false);

        assert_eq!(
            flags.update(ModeFlags::new(true, Some(true), false)),
            changed(true, Some(true), false)
        );
        assert_eq!(
            flags.update(ModeFlags::new(true, Some(false), false)),
            changed(true, Some(false), false)
        );
    }

    #[test]
    fn fullscreen_enter_and_exit() {
        let mut flags = ModeFlags::default();

        assert_eq!(
            flags.update(ModeFlags::new(false, Some(false), true)),
            changed(false, Some(false), true)
        );
        assert_eq!(
            flags.update(ModeFlags::new(false, Some(false), false)),
            changed(false, Some(false), false)
        );
    }

    // The late re-read itself (an `Occluded` after `windowDidExitFullScreen`
    // marks the mode dirty) needs a live macOS window; this pins the diff.
    #[test]
    fn fullscreen_exit_read_late_emits_one_event() {
        let mut flags = ModeFlags::new(false, Some(false), true);
        let exited = ModeFlags::new(false, Some(false), false);

        assert_eq!(flags.update(exited), changed(false, Some(false), false));
        assert_eq!(flags.update(exited), None);
    }

    #[test]
    fn repeated_reads_emit_once() {
        let mut flags = ModeFlags::default();
        let maximized = ModeFlags::new(true, Some(false), false);

        assert!(flags.update(maximized).is_some());
        assert_eq!(flags.update(maximized), None);
        assert_eq!(flags.update(maximized), None);
    }

    #[test]
    fn unknown_minimized_is_not_not_minimized() {
        let mut flags = ModeFlags::new(false, None, false);

        assert_eq!(flags.update(ModeFlags::new(false, None, false)), None);
        assert_eq!(
            flags.update(ModeFlags::new(false, Some(false), false)),
            changed(false, Some(false), false)
        );
        assert_eq!(
            flags.update(ModeFlags::new(false, None, false)),
            changed(false, None, false)
        );
    }
}
