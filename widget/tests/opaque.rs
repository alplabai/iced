//! Tests for the `opaque` helper: it keeps mouse input from reaching the
//! layers below it in a `Stack`.
use iced_test::Simulator;
use iced_widget::core::mouse::{self, ScrollDelta};
use iced_widget::core::widget::{Meta, Tree};
use iced_widget::core::{
    Event, Layout, Length, Point, Rectangle, Shell, Size, Widget, layout, renderer,
};
use iced_widget::{mouse_area, opaque, scrollable, space, stack};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Message {
    BehindScrolled,
    BehindPressed,
    InsideScrolled,
}

/// A full-size area that reports wheel and press events, with an opaque
/// full-size layer on top of it.
fn covered<'a>() -> Simulator<'a, Message> {
    Simulator::new(stack![
        mouse_area(space().width(Length::Fill).height(Length::Fill))
            .on_scroll(|_| Message::BehindScrolled)
            .on_press(Message::BehindPressed),
        opaque(space().width(Length::Fill).height(Length::Fill)),
    ])
}

#[test]
fn wheel_scrolling_does_not_reach_the_layer_below() {
    let mut simulator = covered();
    simulator.point_at(Point::new(100.0, 100.0));

    let _ = simulator.scroll(ScrollDelta::Lines { x: 0.0, y: -1.0 });

    assert_eq!(simulator.into_messages().collect::<Vec<_>>(), []);
}

#[test]
fn mouse_presses_do_not_reach_the_layer_below() {
    let mut simulator = covered();
    simulator.point_at(Point::new(100.0, 100.0));

    let _ = simulator.simulate([Event::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Left,
    ))]);

    assert_eq!(simulator.into_messages().collect::<Vec<_>>(), []);
}

#[test]
fn the_layer_below_still_scrolls_without_the_opaque_layer() {
    let mut simulator: Simulator<'_, Message> = Simulator::new(
        mouse_area(space().width(Length::Fill).height(Length::Fill))
            .on_scroll(|_| Message::BehindScrolled),
    );
    simulator.point_at(Point::new(100.0, 100.0));

    let _ = simulator.scroll(ScrollDelta::Lines { x: 0.0, y: -1.0 });

    assert_eq!(
        simulator.into_messages().collect::<Vec<_>>(),
        [Message::BehindScrolled]
    );
}

/// A widget that reports every wheel event, whether or not the cursor is
/// over it, so only a captured event can keep it quiet.
struct WheelListener;

impl Meta for WheelListener {}

impl<Theme, Renderer> Widget<Message, Theme, Renderer> for WheelListener
where
    Renderer: iced_widget::core::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, Length::Fill, Length::Fill);
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if let Event::Mouse(mouse::Event::WheelScrolled { .. }) = event {
            shell.publish(Message::BehindScrolled);
        }
    }
}

#[test]
fn wheel_scrolling_is_captured_before_cursor_unaware_layers() {
    let mut simulator: Simulator<'_, Message> = Simulator::new(stack![
        WheelListener,
        opaque(space().width(Length::Fill).height(Length::Fill)),
    ]);
    simulator.point_at(Point::new(100.0, 100.0));

    let _ = simulator.scroll(ScrollDelta::Lines { x: 0.0, y: -1.0 });

    assert_eq!(simulator.into_messages().collect::<Vec<_>>(), []);
}

#[test]
fn a_scrollable_below_does_not_scroll() {
    // A scrollable lands the levitated cursor, so only a captured event
    // keeps it from scrolling.
    let mut simulator: Simulator<'_, Message> = Simulator::new(stack![
        scrollable(space().height(3000))
            .width(Length::Fill)
            .height(200)
            .smooth_scroll(false)
            .on_scroll(|_| Message::BehindScrolled),
        opaque(space().width(Length::Fill).height(Length::Fill)),
    ]);
    simulator.point_at(Point::new(100.0, 100.0));

    let _ = simulator.scroll(ScrollDelta::Lines { x: 0.0, y: -1.0 });

    assert_eq!(simulator.into_messages().collect::<Vec<_>>(), []);
}

#[test]
fn content_inside_opaque_still_gets_the_wheel() {
    let mut simulator: Simulator<'_, Message> = Simulator::new(opaque(
        mouse_area(space().width(Length::Fill).height(Length::Fill))
            .on_scroll(|_| Message::InsideScrolled),
    ));
    simulator.point_at(Point::new(100.0, 100.0));

    let _ = simulator.scroll(ScrollDelta::Lines { x: 0.0, y: -1.0 });

    assert_eq!(
        simulator.into_messages().collect::<Vec<_>>(),
        [Message::InsideScrolled]
    );
}

#[test]
fn a_scrollable_scrolls_without_the_opaque_layer() {
    let mut simulator: Simulator<'_, Message> = Simulator::new(
        scrollable(space().height(3000))
            .width(Length::Fill)
            .height(200)
            .smooth_scroll(false)
            .on_scroll(|_| Message::BehindScrolled),
    );
    simulator.point_at(Point::new(100.0, 100.0));

    let _ = simulator.scroll(ScrollDelta::Lines { x: 0.0, y: -1.0 });

    assert_eq!(
        simulator.into_messages().collect::<Vec<_>>(),
        [Message::BehindScrolled]
    );
}
