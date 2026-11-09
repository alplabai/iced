//! A native macOS menu bar built from the state, and a quit that waits for
//! unsaved changes.
//!
//! Edit the text, then press Command+Q, choose Quit from the app menu or the
//! Dock, or close the window: the app asks before it quits. On other
//! platforms the menu is ignored and only closing the window asks.
use iced::menu::{Menu, Predefined, item, item_maybe, predefined, separator, submenu};
use iced::widget::{button, center, column, row, text, text_input};
use iced::{Element, Settings, Subscription, Task, Widget, system, window};

pub fn main() -> iced::Result {
    iced::application(Editor::default, Editor::update, Editor::view)
        .settings(Settings {
            // Quit requests reach `system::quit_requests` instead of quitting.
            exit_on_quit_request: false,
            ..Settings::default()
        })
        // Closing the window asks too, instead of exiting at once.
        .exit_on_close_request(false)
        .menu(Editor::menu)
        .subscription(Editor::subscription)
        .run()
}

#[derive(Default)]
struct Editor {
    content: String,
    saved: String,
    is_word_wrap: bool,
    is_confirming_quit: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(String),
    Save,
    Revert,
    ToggleWordWrap,
    QuitRequested,
    MenuFailed(String),
    ConfirmQuit,
    CancelQuit,
}

impl Editor {
    fn is_dirty(&self) -> bool {
        self.content != self.saved
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(content) => self.content = content,
            Message::Save => self.saved = self.content.clone(),
            Message::Revert => self.content = self.saved.clone(),
            Message::ToggleWordWrap => self.is_word_wrap = !self.is_word_wrap,
            // A real app would show an in-window menu here.
            Message::MenuFailed(reason) => {
                eprintln!("The native menu could not be built: {reason}")
            }
            // Every quit request needs an answer: quit now, or ask first.
            Message::QuitRequested if self.is_dirty() => self.is_confirming_quit = true,
            Message::QuitRequested | Message::ConfirmQuit => return iced::exit(),
            Message::CancelQuit => {
                self.is_confirming_quit = false;
                return system::cancel_quit();
            }
        }

        Task::none()
    }

    fn menu(&self) -> Menu<Message> {
        let dirty = self.is_dirty();

        Menu::new([
            // The first submenu is the app menu; macOS titles it.
            submenu(
                "App",
                [
                    predefined(Predefined::About),
                    separator(),
                    predefined(Predefined::Services),
                    separator(),
                    predefined(Predefined::Hide),
                    predefined(Predefined::HideOthers),
                    predefined(Predefined::ShowAll),
                    separator(),
                    predefined(Predefined::Quit),
                ],
            ),
            submenu(
                "File",
                [
                    item_maybe("Save", dirty.then_some(Message::Save)).shortcut("CmdOrCtrl+S"),
                    item_maybe("Revert", dirty.then_some(Message::Revert)),
                ],
            ),
            // No Edit menu: system Edit rows would take Command+C/V away from
            // the text input (see `Predefined`).
            submenu(
                "View",
                [item("Word Wrap", Message::ToggleWordWrap).checked(self.is_word_wrap)],
            ),
            submenu(
                "Window",
                [
                    predefined(Predefined::Minimize),
                    predefined(Predefined::Zoom),
                    predefined(Predefined::Fullscreen),
                ],
            ),
        ])
    }

    fn subscription(&self) -> Subscription<Message> {
        // An app quit (macOS) and a window close lead to the same question.
        Subscription::batch([
            system::quit_requests().map(|()| Message::QuitRequested),
            window::close_requests().map(|_| Message::QuitRequested),
            system::menu_failures().map(Message::MenuFailed),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let status = if self.is_dirty() {
            "Unsaved changes"
        } else {
            "Saved"
        };
        let input = text_input("Type something...", &self.content).on_input(Message::Edit);
        let word_wrap = text(format!("Word wrap: {}", self.is_word_wrap));

        let body = if self.is_confirming_quit {
            column![
                input,
                text(status),
                word_wrap,
                text("Quit and discard the unsaved changes?"),
                row![
                    button("Cancel").on_press(Message::CancelQuit),
                    button("Quit").on_press(Message::ConfirmQuit),
                ]
                .spacing(10),
            ]
        } else {
            column![input, text(status), word_wrap]
        };

        center(body.spacing(10).width(400)).padding(20).boxed()
    }
}
