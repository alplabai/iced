//! Which windows are disabled, and which events a disabled window still gets.
use crate::core::{self, Event, keyboard, mouse, window};

use std::collections::BTreeSet;

/// The windows the application disabled with `window::set_enabled`.
#[derive(Debug, Default)]
pub struct Disabled(BTreeSet<window::Id>);

impl Disabled {
    pub fn set_enabled(&mut self, id: window::Id, enabled: bool) {
        if enabled {
            let _ = self.0.remove(&id);
        } else {
            let _ = self.0.insert(id);
        }
    }

    pub fn is_enabled(&self, id: window::Id) -> bool {
        !self.0.contains(&id)
    }

    pub fn forget(&mut self, id: window::Id) {
        let _ = self.0.remove(&id);
    }

    /// If windows remain open and every one of them is disabled, the user
    /// could not interact with the application any more: enables them all
    /// and returns them.
    pub fn reenable_stranded(&mut self, open: &[window::Id]) -> Vec<window::Id> {
        if open.is_empty() || open.iter().any(|id| self.is_enabled(*id)) {
            return Vec::new();
        }

        for id in open {
            self.forget(*id);
        }

        open.to_vec()
    }
}

/// Whether a disabled window still receives `event`.
///
/// Window lifecycle and layout events pass, so the window keeps drawing and
/// can be closed; everything the user does with the pointer, keyboard, touch,
/// IME or dropped files is dropped.
pub fn passes_when_disabled(event: &Event) -> bool {
    match event {
        Event::Window(event) => !matches!(
            event,
            window::Event::FileHovered(_)
                | window::Event::FileDropped(_)
                | window::Event::FilesHoveredLeft
        ),
        Event::Keyboard(event) => matches!(event, keyboard::Event::ModifiersChanged(_)),
        Event::Clipboard(_) | Event::Waken => true,
        Event::Mouse(_) | Event::Touch(_) | Event::InputMethod(_) | Event::Gesture(_) => false,
    }
}

/// Whether the dropped `event` shows the user tried to use a disabled window.
pub fn is_press(event: &Event) -> bool {
    matches!(
        event,
        Event::Mouse(mouse::Event::ButtonPressed(_))
            | Event::Touch(core::touch::Event::FingerPressed { .. })
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::core::{Point, Size, input_method, touch};
    use std::path::PathBuf;

    #[test]
    fn disabled_flag_set_and_cleared_by_set_enabled() {
        let id = window::Id::unique();
        let mut disabled = Disabled::default();

        assert!(disabled.is_enabled(id));
        disabled.set_enabled(id, false);
        assert!(!disabled.is_enabled(id));
        disabled.set_enabled(id, true);
        assert!(disabled.is_enabled(id));
    }

    #[test]
    fn removed_window_forgets_disabled_flag() {
        let id = window::Id::unique();
        let mut disabled = Disabled::default();

        disabled.set_enabled(id, false);
        disabled.forget(id);

        assert!(disabled.is_enabled(id));
    }

    #[test]
    fn closing_last_enabled_window_reenables_the_rest() {
        let (owner, other) = (window::Id::unique(), window::Id::unique());
        let mut disabled = Disabled::default();

        disabled.set_enabled(owner, false);
        disabled.set_enabled(other, false);

        let mut reenabled = disabled.reenable_stranded(&[owner, other]);
        reenabled.sort();

        assert_eq!(reenabled, [owner, other]);
        assert!(disabled.is_enabled(owner) && disabled.is_enabled(other));
        assert!(disabled.reenable_stranded(&[]).is_empty());
    }

    #[test]
    fn nested_modal_keeps_owner_disabled() {
        let (root, dialog) = (window::Id::unique(), window::Id::unique());
        let mut disabled = Disabled::default();

        disabled.set_enabled(root, false);

        assert!(disabled.reenable_stranded(&[root, dialog]).is_empty());
        assert!(!disabled.is_enabled(root));
    }

    #[test]
    fn disabled_windows_keep_lifecycle_events_and_drop_user_input() {
        let keep = [
            Event::Window(window::Event::CloseRequested),
            Event::Window(window::Event::Focused),
            Event::Window(window::Event::Unfocused),
            Event::Window(window::Event::Resized(Size::new(1.0, 1.0))),
            Event::Window(window::Event::Moved(Point::ORIGIN)),
            Event::Window(window::Event::Closed),
            Event::Window(window::Event::Rescaled(2.0)),
            Event::Keyboard(keyboard::Event::ModifiersChanged(
                keyboard::Modifiers::SHIFT,
            )),
            Event::Waken,
        ];

        let drop = [
            Event::Window(window::Event::FileHovered(PathBuf::new())),
            Event::Window(window::Event::FileDropped(PathBuf::new())),
            Event::Window(window::Event::FilesHoveredLeft),
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::ORIGIN,
            }),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
            Event::Mouse(mouse::Event::CursorLeft),
            Event::Touch(touch::Event::FingerPressed {
                id: touch::Finger(0),
                position: Point::ORIGIN,
            }),
            Event::InputMethod(input_method::Event::Opened),
        ];

        assert!(keep.iter().all(passes_when_disabled));
        assert!(!drop.iter().any(passes_when_disabled));
        assert!(is_press(&drop[4]) && is_press(&drop[8]) && !is_press(&drop[5]));
    }
}
