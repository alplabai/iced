//! Tests that containers skip children outside of the viewport when asking
//! for the mouse interaction, as `draw` already does.
use iced_widget::core::layout;
use iced_widget::core::mouse;
use iced_widget::core::renderer;
use iced_widget::core::widget::{Meta, Tree, tree};
use iced_widget::core::{Layout, Length, Never, Point, Rectangle, Size, Widget};
use iced_widget::{Theme, column, row};

use std::cell::Cell;
use std::rc::Rc;

/// A fixed-size widget that counts how many times it is asked for its
/// mouse interaction.
struct Probe {
    size: f32,
    calls: Rc<Cell<usize>>,
}

impl Meta for Probe {}

impl Widget<Never, Theme, ()> for Probe {
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &(), limits: &layout::Limits) {
        tree.size = limits.resolve(
            Length::Fixed(self.size),
            Length::Fixed(self.size),
            Size::ZERO,
        );
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut (),
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        _layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &(),
    ) -> mouse::Interaction {
        self.calls.set(self.calls.get() + 1);

        mouse::Interaction::None
    }
}

fn probes(count: usize, calls: &Rc<Cell<usize>>) -> Vec<Probe> {
    (0..count)
        .map(|_| Probe {
            size: 10.0,
            calls: calls.clone(),
        })
        .collect()
}

/// Lays out `widget`, then asks it for its mouse interaction with the given
/// viewport.
fn interact(mut widget: impl Widget<Never, Theme, ()>, viewport: Rectangle) {
    let mut tree = Tree::new(&widget);
    widget.diff(&mut tree);

    let limits = layout::Limits::new(Size::ZERO, Size::new(1000.0, 1000.0));
    widget.layout(&mut tree, &(), &limits);

    let layout = Layout::new(tree.size);
    let cursor = mouse::Cursor::Available(Point::new(1.0, 1.0));

    let _ = widget.mouse_interaction(&tree, layout, cursor, &viewport, &());
}

/// Only the 5 probes of a 1000 probe column that intersect the viewport are
/// asked for their mouse interaction.
#[test]
fn column_skips_children_outside_of_viewport() {
    let calls = Rc::new(Cell::new(0));
    let widget = column(probes(1000, &calls)).width(Length::Shrink);

    interact(widget, Rectangle::new(Point::ORIGIN, Size::new(10.0, 50.0)));

    assert_eq!(calls.get(), 5);
}

/// Same for a `row`.
#[test]
fn row_skips_children_outside_of_viewport() {
    let calls = Rc::new(Cell::new(0));
    let widget = row(probes(1000, &calls)).height(Length::Shrink);

    interact(widget, Rectangle::new(Point::ORIGIN, Size::new(50.0, 10.0)));

    assert_eq!(calls.get(), 5);
}
