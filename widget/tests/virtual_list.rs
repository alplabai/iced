//! Tests for the `list::VirtualList` widget.
use iced_test::Simulator;
use iced_widget::core::layout;
use iced_widget::core::mouse::{self, ScrollDelta};
use iced_widget::core::renderer;
use iced_widget::core::widget::operation::{Animation, scrollable::AbsoluteOffset};
use iced_widget::core::widget::{Id, Meta, Tree, operation, tree};
use iced_widget::core::{Layout, Length, Never, Rectangle, Size, Theme, Widget};
use iced_widget::list::{self, VirtualList};
use iced_widget::text;

use std::cell::Cell;

fn simulate<'a>(
    element: impl Widget<(), Theme, iced_widget::Renderer> + 'a,
) -> Simulator<'a, (), Theme, iced_widget::Renderer> {
    Simulator::new(element)
}

/// A stateful leaf widget. Its state records the id it was created with and
/// is never overwritten by `diff`, so it reveals which tree a widget was
/// matched with.
struct Probe(u64);

struct ProbeState(u64);

impl Meta for Probe {}

impl Widget<Never, Theme, ()> for Probe {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ProbeState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ProbeState(self.0))
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &(), limits: &layout::Limits) {
        tree.size = limits.max;
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
}

/// Lays out the list in a 100x100 viewport and returns the ids stored in
/// the state of each built row.
fn states(list: &mut impl Widget<Never, Theme, ()>, tree: &mut Tree) -> Vec<u64> {
    list.layout(
        tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );

    tree.children
        .iter()
        .map(|child| child.state.downcast_ref::<ProbeState>().0)
        .collect()
}

fn state_ids(tree: &Tree) -> Vec<u64> {
    tree.children
        .iter()
        .map(|child| child.state.downcast_ref::<ProbeState>().0)
        .collect()
}

#[test]
fn builds_only_the_visible_rows() {
    for len in [50_000, 100_000] {
        let built = Cell::new(0);

        let mut list = VirtualList::new(len, 10.0, |i| {
            built.set(built.get() + 1);
            Probe(i as u64)
        });

        let mut tree = Tree::new::<Never, Theme, ()>(&list);
        let _ = states(&mut list, &mut tree);

        // 10 visible rows plus 2 of overscan below the viewport.
        assert_eq!(built.get(), 12, "len = {len}");
        assert_eq!(tree.children.len(), 12);
    }
}

#[test]
fn empty_and_short_lists_build_what_exists() {
    let mut empty = VirtualList::new(0, 10.0, |i| Probe(i as u64));
    let mut tree = Tree::new::<Never, Theme, ()>(&empty);
    assert!(states(&mut empty, &mut tree).is_empty());

    let mut short = VirtualList::new(3, 10.0, |i| Probe(i as u64));
    let mut tree = Tree::new::<Never, Theme, ()>(&short);
    assert_eq!(states(&mut short, &mut tree), vec![0, 1, 2]);
}

#[test]
fn state_follows_key_when_rows_reorder() {
    // Row index `i` currently shows the item `ITEMS[i]`; the key is the item.
    let first = [10_u64, 11, 12];
    let mut list = VirtualList::new(3, 10.0, |i| Probe(first[i])).key(|i| first[i]);
    let mut tree = Tree::new::<Never, Theme, ()>(&list);
    assert_eq!(states(&mut list, &mut tree), vec![10, 11, 12]);

    // The items are reordered. A positional diff would hand key 12 the state
    // of key 10. The `Probe`s here are built with a *different* id so that a
    // fresh state would be visible.
    let second = [12_u64, 10, 11];
    let mut list = VirtualList::new(3, 10.0, |_| Probe(999)).key(|i| second[i]);
    list.layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );

    assert_eq!(state_ids(&tree), vec![12, 10, 11]);
}

#[test]
fn state_follows_key_when_the_window_slides() {
    let mut list = VirtualList::new(100, 10.0, |i| Probe(i as u64)).overscan(0);
    let mut tree = Tree::new::<Never, Theme, ()>(&list);
    assert_eq!(states(&mut list, &mut tree), (0..10).collect::<Vec<_>>());

    // Scroll by 5 rows. Rows 5..10 survive and must keep their state, while
    // rows 10..15 are new. The closure builds every row with a different id
    // so that a fresh state would be visible.
    let mut operation = operation::scrollable::scroll_by::<()>(
        Id::new("list"),
        AbsoluteOffset { x: 0.0, y: 50.0 },
        Animation::Instant,
    );

    let mut list = VirtualList::new(100, 10.0, |i| Probe(1_000 + i as u64))
        .id("list")
        .overscan(0);

    let layout = Layout::new(tree.size);

    list.operate(
        &mut tree,
        layout,
        &Rectangle::with_size(Size::new(100.0, 100.0)),
        &(),
        &mut operation,
    );

    assert_eq!(
        states(&mut list, &mut tree),
        vec![5, 6, 7, 8, 9, 1_010, 1_011, 1_012, 1_013, 1_014]
    );
}

#[test]
fn reveal_row_scrolls_an_offscreen_row_into_view() {
    let id = Id::new("list");

    let element = VirtualList::new(1_000, 20.0, |i| text::<Theme>(format!("Row {i}")))
        .id(id.clone())
        .height(100);

    let mut simulator = simulate(element);

    assert!(simulator.find("Row 500").is_err());

    simulator.operate(&mut list::reveal_row::<()>(id, 500, 20.0));

    // Operations only change the state; the next event rebuilds the rows.
    let _ = simulator.simulate([iced_widget::core::Event::Mouse(mouse::Event::CursorMoved {
        position: iced_widget::core::Point::new(1.0, 1.0),
    })]);

    assert!(simulator.find("Row 500").is_ok());
    assert!(simulator.find("Row 0").is_err());
}

#[test]
fn wheel_scrolls_the_list() {
    let element = VirtualList::new(1_000, 20.0, |i| text::<Theme>(format!("Row {i}"))).height(100);

    let mut simulator = simulate(element);
    simulator.point_at(iced_widget::core::Point::new(5.0, 5.0));

    assert!(simulator.find("Row 0").is_ok());
    assert!(simulator.find("Row 40").is_err());

    // One line scrolls one row.
    let _ = simulator.scroll(ScrollDelta::Lines { x: 0.0, y: -40.0 });

    assert!(simulator.find("Row 0").is_err());
    assert!(simulator.find("Row 40").is_ok());
}
