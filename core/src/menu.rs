//! Describe the native menu bar of an application.
//!
//! A [`Menu`] is plain data built from the application state, like a view:
//! the runtime rebuilds it after every update and only touches the native
//! menu when something changed.
//!
//! Only macOS has a native menu bar today; on other platforms a [`Menu`] is
//! ignored.

/// The menu bar of an application: a list of top-level [`Item`]s.
///
/// On macOS every top-level item should be a [`submenu`]; the first one is
/// the application menu and is titled by the system with the process name.
///
/// A [`Menu`] replaces the default menu bar entirely, including its Command+Q
/// and Command+H. Put [`Predefined::Quit`], [`Predefined::Hide`],
/// [`Predefined::HideOthers`] and [`Predefined::ShowAll`] in the first
/// submenu to keep them.
///
/// The menu is delivered through `muda`'s process-wide event handler, which
/// iced installs; another `muda` menu in the same application will not
/// receive its events.
#[derive(Debug, Clone, PartialEq)]
pub struct Menu<Message> {
    items: Vec<Item<Message>>,
}

impl<Message> Menu<Message> {
    /// Creates a [`Menu`] with the given top-level items.
    pub fn new(items: impl IntoIterator<Item = Item<Message>>) -> Self {
        Self {
            items: items.into_iter().collect(),
        }
    }

    /// Returns the top-level items of the [`Menu`].
    pub fn items(&self) -> &[Item<Message>] {
        &self.items
    }

    /// Consumes the [`Menu`] and returns its top-level items.
    pub fn into_items(self) -> Vec<Item<Message>> {
        self.items
    }

    /// Maps the messages produced by the [`Menu`].
    pub fn map<B>(self, f: impl Fn(Message) -> B + Clone) -> Menu<B> {
        Menu {
            items: self
                .items
                .into_iter()
                .map(|item| item.map(f.clone()))
                .collect(),
        }
    }
}

/// An entry of a [`Menu`].
#[derive(Debug, Clone, PartialEq)]
pub enum Item<Message> {
    /// A row that produces a message when chosen.
    Action(Action<Message>),
    /// A row that opens a nested list of items.
    Submenu(Submenu<Message>),
    /// A horizontal divider.
    Separator,
    /// A row whose behavior is provided by the operating system.
    Predefined(Predefined),
}

impl<Message> Item<Message> {
    /// Sets the check mark of an [`Item::Action`]; `None` shows no mark.
    ///
    /// It has no effect on other kinds of items.
    pub fn checked(self, checked: impl Into<Option<bool>>) -> Self {
        match self {
            Item::Action(action) => Item::Action(Action {
                checked: checked.into(),
                ..action
            }),
            item => item,
        }
    }

    /// Sets the keyboard shortcut of an [`Item::Action`], for example
    /// `"CmdOrCtrl+Shift+S"`.
    ///
    /// The shortcut is registered with the operating system and fires
    /// before the application sees the key press, so a focused text input
    /// will not receive it either. Prefer showing the shortcut in the label
    /// and handling the key in the application when that matters.
    ///
    /// An invalid shortcut is ignored and logged.
    ///
    /// It has no effect on other kinds of items.
    pub fn shortcut(self, shortcut: impl Into<String>) -> Self {
        match self {
            Item::Action(action) => Item::Action(Action {
                shortcut: Some(shortcut.into()),
                ..action
            }),
            item => item,
        }
    }

    /// Maps the message produced by the [`Item`].
    pub fn map<B>(self, f: impl Fn(Message) -> B + Clone) -> Item<B> {
        match self {
            Item::Action(action) => Item::Action(Action {
                label: action.label,
                on_activate: action.on_activate.map(f),
                checked: action.checked,
                shortcut: action.shortcut,
            }),
            Item::Submenu(submenu) => Item::Submenu(Submenu {
                label: submenu.label,
                enabled: submenu.enabled,
                items: submenu
                    .items
                    .into_iter()
                    .map(|item| item.map(f.clone()))
                    .collect(),
            }),
            Item::Separator => Item::Separator,
            Item::Predefined(predefined) => Item::Predefined(predefined),
        }
    }
}

/// A row that produces a message when chosen.
#[derive(Debug, Clone, PartialEq)]
pub struct Action<Message> {
    /// The text of the row.
    pub label: String,
    /// The message produced when the row is chosen; `None` greys it out.
    pub on_activate: Option<Message>,
    /// The check mark; `None` shows no mark.
    pub checked: Option<bool>,
    /// The keyboard shortcut registered with the operating system.
    pub shortcut: Option<String>,
}

/// A row that opens a nested list of items.
#[derive(Debug, Clone, PartialEq)]
pub struct Submenu<Message> {
    /// The text of the row.
    pub label: String,
    /// Whether the submenu can be opened.
    pub enabled: bool,
    /// The nested items.
    pub items: Vec<Item<Message>>,
}

/// A row whose behavior is provided by the operating system.
///
/// There are no Edit rows (Undo, Cut, Copy, Paste, Select All): the system
/// sends those to native text views, which iced widgets are not, so they would
/// do nothing and their key equivalents would take Command+C/V/X/A/Z away from
/// `text_input` and `text_editor`. Use an [`item`] that sends your own message
/// and show the shortcut in its label instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Predefined {
    /// About the application.
    About,
    /// The system Services submenu (macOS).
    Services,
    /// Hide the application (macOS).
    Hide,
    /// Hide the other applications (macOS).
    HideOthers,
    /// Show all applications (macOS).
    ShowAll,
    /// Quit the application.
    ///
    /// On macOS this asks to terminate; with
    /// [`Settings::exit_on_quit_request`](crate::Settings::exit_on_quit_request)
    /// set to `false` the request reaches the application instead.
    Quit,
    /// Minimize the focused window.
    Minimize,
    /// Zoom the focused window (macOS).
    Zoom,
    /// Toggle full screen for the focused window (macOS).
    Fullscreen,
    /// Close the focused window.
    CloseWindow,
    /// Bring all windows to the front (macOS).
    BringAllToFront,
}

/// Creates an enabled [`Item::Action`] that produces `on_activate`.
pub fn item<Message>(label: impl Into<String>, on_activate: Message) -> Item<Message> {
    item_maybe(label, Some(on_activate))
}

/// Creates an [`Item::Action`] that is greyed out when `on_activate` is `None`.
pub fn item_maybe<Message>(
    label: impl Into<String>,
    on_activate: Option<Message>,
) -> Item<Message> {
    Item::Action(Action {
        label: label.into(),
        on_activate,
        checked: None,
        shortcut: None,
    })
}

/// Creates an enabled [`Item::Submenu`].
pub fn submenu<Message>(
    label: impl Into<String>,
    items: impl IntoIterator<Item = Item<Message>>,
) -> Item<Message> {
    Item::Submenu(Submenu {
        label: label.into(),
        enabled: true,
        items: items.into_iter().collect(),
    })
}

/// Creates an [`Item::Separator`].
pub fn separator<Message>() -> Item<Message> {
    Item::Separator
}

/// Creates an [`Item::Predefined`].
pub fn predefined<Message>(predefined: Predefined) -> Item<Message> {
    Item::Predefined(predefined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_and_shortcut_only_apply_to_actions() {
        let action = item("Save", 1).checked(true).shortcut("CmdOrCtrl+S");
        let Item::Action(action) = action else {
            panic!("expected an action");
        };
        assert_eq!(action.checked, Some(true));
        assert_eq!(action.shortcut.as_deref(), Some("CmdOrCtrl+S"));

        assert_eq!(separator::<i32>().checked(true), Item::Separator);
    }

    #[test]
    fn item_maybe_none_is_disabled() {
        let Item::Action(action) = item_maybe::<i32>("Save", None) else {
            panic!("expected an action");
        };
        assert_eq!(action.on_activate, None);
    }

    #[test]
    fn map_reaches_nested_items() {
        let menu = Menu::new([submenu("File", [submenu("Export", [item("PDF", 2)])])]);
        let mapped = menu.map(|n| n * 10);

        assert_eq!(
            mapped,
            Menu::new([submenu("File", [submenu("Export", [item("PDF", 20)])])])
        );
    }
}
