/// The way the mouse cursor is held by a window.
///
/// See [`window::set_cursor_grab`] in `iced_runtime` for how to request one.
///
/// [`window::set_cursor_grab`]: https://docs.rs/iced_runtime/latest/iced_runtime/window/fn.set_cursor_grab.html
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorGrab {
    /// The cursor moves freely and can leave the window.
    #[default]
    None,
    /// The cursor is confined to the window area, but still moves and is
    /// still visible.
    Confined,
    /// The cursor is locked in place and hidden, so it cannot reach the edge
    /// of the screen.
    Locked,
}

impl CursorGrab {
    /// Returns the next weaker mode to try when the platform does not
    /// support this one, or [`None`](Option::None) if there is nothing
    /// weaker than [`CursorGrab::None`].
    ///
    /// The chain is `Locked`, then `Confined`, then `None`.
    pub fn fallback(self) -> Option<Self> {
        match self {
            Self::Locked => Some(Self::Confined),
            Self::Confined => Some(Self::None),
            Self::None => None,
        }
    }

    /// Returns `true` if the cursor is held in some way, that is, if the mode
    /// is not [`CursorGrab::None`].
    pub fn is_grabbed(self) -> bool {
        self != Self::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_none() {
        assert_eq!(CursorGrab::default(), CursorGrab::None);
        assert!(!CursorGrab::None.is_grabbed());
    }

    #[test]
    fn grabbed_modes() {
        assert!(CursorGrab::Confined.is_grabbed());
        assert!(CursorGrab::Locked.is_grabbed());
    }

    #[test]
    fn fallback_chain_ends_at_none() {
        let chain: Vec<_> =
            std::iter::successors(Some(CursorGrab::Locked), |mode| mode.fallback()).collect();

        assert_eq!(
            chain,
            [CursorGrab::Locked, CursorGrab::Confined, CursorGrab::None]
        );
    }
}
