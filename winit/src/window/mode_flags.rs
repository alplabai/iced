use crate::core::window;

/// The maximized, minimized and fullscreen flags of a window.
///
/// It remembers the last known flags so that a change can be detected
/// without ever comparing window sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModeFlags {
    maximized: bool,
    minimized: bool,
    fullscreen: bool,
}

impl ModeFlags {
    /// Creates [`ModeFlags`] from the given flags.
    pub fn new(maximized: bool, minimized: bool, fullscreen: bool) -> Self {
        Self {
            maximized,
            minimized,
            fullscreen,
        }
    }

    /// Reads the current [`ModeFlags`] of a [`winit::window::Window`].
    pub fn read(window: &winit::window::Window) -> Self {
        Self::new(
            window.is_maximized(),
            window.is_minimized().unwrap_or(false),
            window.fullscreen().is_some(),
        )
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

    fn changed(maximized: bool, minimized: bool, fullscreen: bool) -> Option<window::Event> {
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

        let mut flags = ModeFlags::new(true, false, false);
        assert_eq!(flags.update(ModeFlags::new(true, false, false)), None);
    }

    #[test]
    fn maximize_and_unmaximize() {
        let mut flags = ModeFlags::default();

        assert_eq!(
            flags.update(ModeFlags::new(true, false, false)),
            changed(true, false, false)
        );
        assert_eq!(
            flags.update(ModeFlags::new(false, false, false)),
            changed(false, false, false)
        );
    }

    #[test]
    fn minimize_and_restore() {
        let mut flags = ModeFlags::new(true, false, false);

        assert_eq!(
            flags.update(ModeFlags::new(true, true, false)),
            changed(true, true, false)
        );
        assert_eq!(
            flags.update(ModeFlags::new(true, false, false)),
            changed(true, false, false)
        );
    }

    #[test]
    fn fullscreen_enter_and_exit() {
        let mut flags = ModeFlags::default();

        assert_eq!(
            flags.update(ModeFlags::new(false, false, true)),
            changed(false, false, true)
        );
        assert_eq!(
            flags.update(ModeFlags::new(false, false, false)),
            changed(false, false, false)
        );
    }

    #[test]
    fn repeated_reads_emit_once() {
        let mut flags = ModeFlags::default();
        let maximized = ModeFlags::new(true, false, false);

        assert!(flags.update(maximized).is_some());
        assert_eq!(flags.update(maximized), None);
        assert_eq!(flags.update(maximized), None);
    }
}
