//! The title bar regions a [`UserInterface`] reports after a redraw.
use iced_runtime::core;
use iced_runtime::core::layout;
use iced_runtime::core::overlay;
use iced_runtime::core::renderer;
use iced_runtime::core::shell;
use iced_runtime::core::time::Instant;
use iced_runtime::core::widget;
use iced_runtime::core::window;
use iced_runtime::core::{Event, Length, Rectangle, Shell, Size, Widget};
use iced_runtime::user_interface::{Cache, State, UserInterface};

type Renderer = ();

const CAPTION: Rectangle = Rectangle {
    x: 120.0,
    y: 0.0,
    width: 300.0,
    height: 36.0,
};

/// Reports [`CAPTION`] on every redraw, and shows an empty overlay when asked.
struct Strip {
    overlay: bool,
}

impl Widget<(), core::Theme, Renderer> for Strip {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut widget::Tree, _renderer: &Renderer, _limits: &layout::Limits) {
        tree.size = Size::new(800.0, 600.0);
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        _renderer: &mut Renderer,
        _theme: &core::Theme,
        _style: &renderer::Style,
        _layout: layout::Layout,
        _cursor: core::mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn update(
        &mut self,
        _tree: &mut widget::Tree,
        event: &Event,
        _layout: layout::Layout,
        _cursor: core::mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, ()>,
        _viewport: &Rectangle,
    ) {
        if let Event::Window(window::Event::RedrawRequested(_)) = event {
            shell.request_title_bar_region(window::TitleBarRegion::Caption, CAPTION);
        }
    }

    fn overlay<'a>(
        &'a mut self,
        _tree: &'a mut widget::Tree,
        _layout: layout::Layout,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        _translation: core::Vector,
        _window: core::Size,
    ) -> Vec<overlay::Element<'a, (), core::Theme, Renderer>> {
        if self.overlay {
            vec![overlay::Element::new(Box::new(Popup))]
        } else {
            Vec::new()
        }
    }
}

impl widget::Meta for Strip {}

struct Popup;

impl overlay::Overlay<(), core::Theme, Renderer> for Popup {
    fn draw(
        &self,
        _renderer: &mut Renderer,
        _theme: &core::Theme,
        _style: &renderer::Style,
        _cursor: core::mouse::Cursor,
    ) {
    }
}

fn caption_after_redraw(overlay: bool) -> Vec<Rectangle> {
    let mut renderer = ();
    let mut user_interface = UserInterface::build(
        Strip { overlay },
        Size::new(800.0, 600.0),
        Cache::default(),
        &mut renderer,
    );

    let (state, _) = user_interface.update(
        &window::Headless,
        &shell::Waker::noop(),
        &[Event::Window(
            window::Event::RedrawRequested(Instant::now()),
        )],
        core::mouse::Cursor::Unavailable,
        &mut renderer,
        &mut shell::Bus::new(),
    );

    match state {
        State::Updated { title_bar, .. } => title_bar.caption,
        State::Outdated => panic!("the user interface should stay up to date"),
    }
}

#[test]
fn a_redraw_reports_the_caption_regions() {
    assert_eq!(caption_after_redraw(false), vec![CAPTION]);
}

#[test]
fn an_open_overlay_suspends_the_caption_regions() {
    assert_eq!(caption_after_redraw(true), Vec::<Rectangle>::new());
}
