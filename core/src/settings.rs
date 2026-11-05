//! Configure your application.
use crate::backend;
use crate::renderer;
use crate::text;
use crate::{Backend, Font, Pixels};

use std::borrow::Cow;

/// The settings of an iced program.
#[derive(Debug, Clone)]
pub struct Settings {
    /// The identifier of the application.
    ///
    /// If provided, this identifier may be used to identify the application or
    /// communicate with it through the windowing system.
    pub id: Option<String>,

    /// The fonts to load on boot.
    pub fonts: Vec<Cow<'static, [u8]>>,

    /// The default [`Font`] to be used.
    ///
    /// By default, it uses [`Family::SansSerif`](crate::font::Family::SansSerif).
    pub font: Font,

    /// The text size that will be used by default.
    ///
    /// By default, it is `16.0`.
    pub text_size: Pixels,

    /// The default line height of text.
    ///
    /// By default, it is `LineHeight::Relative(1.375)`.
    pub line_height: text::LineHeight,

    /// Whether certain widgets should be rendered using metrics hinting.
    ///
    /// Metrics hinting can improve the readability of smaller text in
    /// low-DPI screens, as well as the clarity of widgets that render thin lines.
    ///
    /// By default, it is enabled.
    pub metrics_hinting: bool,

    /// The graphical backend to use.
    ///
    /// By default, it is [`Backend::Best`].
    pub backend: Backend,

    /// The [`PowerPreference`](backend::PowerPreference) of the backend.
    ///
    /// By default, it is [`backend::PowerPreference::None`].
    pub power_preference: backend::PowerPreference,

    /// If set to true, the renderer will try to perform antialiasing for some
    /// primitives.
    ///
    /// Enabling it can produce a smoother result in some widgets, like the
    /// `canvas` widget, at a performance cost.
    ///
    /// By default, it is enabled.
    pub antialiasing: bool,

    /// Whether or not to attempt to synchronize rendering when possible.
    ///
    /// Disabling it can improve rendering performance on some platforms.
    ///
    /// By default, it is enabled.
    pub vsync: bool,

    /// Whether a request to quit the application (on macOS: Command+Q,
    /// Dock ▸ Quit, logout, [`menu::Predefined::Quit`](crate::menu::Predefined::Quit))
    /// quits it at once.
    ///
    /// When `false`, the request reaches the application through
    /// `system::quit_requests`, which must answer it with `exit` (quit) or
    /// `system::cancel_quit` (stay). Until it answers, the system waits; at
    /// logout macOS only waits a limited time before it reports that the
    /// application is blocking logout.
    ///
    /// Only has an effect on macOS. By default, it is enabled.
    pub exit_on_quit_request: bool,

    /// Whether the windowing system installs its default menu bar.
    ///
    /// Turn it off when the application installs its own native menu bar
    /// without using a [`Menu`](crate::menu::Menu). A program that provides a
    /// [`Menu`](crate::menu::Menu) does not need to: its menu replaces the
    /// default one once the application has launched.
    ///
    /// Only has an effect on macOS. By default, it is enabled.
    pub default_menu: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let renderer = renderer::Settings::default();

        Self {
            id: None,
            fonts: Vec::new(),
            font: renderer.font,
            text_size: renderer.text_size,
            line_height: renderer.line_height,
            metrics_hinting: true,
            backend: Backend::default(),
            power_preference: backend::PowerPreference::None,
            antialiasing: true,
            vsync: true,
            exit_on_quit_request: true,
            default_menu: true,
        }
    }
}

impl From<&Settings> for renderer::Settings {
    fn from(settings: &Settings) -> Self {
        Self {
            font: settings.font,
            text_size: settings.text_size,
            line_height: settings.line_height,
            metrics_hinting: settings.metrics_hinting,
        }
    }
}
