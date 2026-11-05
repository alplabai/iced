//! Tests for `push_boxed` and `extend_boxed` on the layout containers.
use iced_widget::core::widget;
use iced_widget::core::{Element, Never, Widget};
use iced_widget::{Column, Grid, Renderer, Row, Stack, Theme, button, keyed, space, text};

type El<'a> = Element<'a, Never, Theme, Renderer>;

fn count(widget: &mut impl Widget<Never, Theme, Renderer>) -> usize {
    let mut tree = widget::Tree::new(&*widget);
    widget.diff(&mut tree);

    tree.children.len()
}

/// A row can be built from three different widget types without `.boxed()`.
#[test]
fn row_accepts_heterogeneous_children() {
    let mut row: Row<El<'_>> = Row::new()
        .push_boxed(text("a"))
        .push_boxed(button("b"))
        .push_boxed(space());

    assert_eq!(count(&mut row), 3);
}

/// `None` children are filtered out, as with `push`.
#[test]
fn none_children_add_nothing() {
    let none = || None::<iced_widget::Text<'static>>;

    assert_eq!(
        count(
            &mut Row::<El<'_>>::new()
                .push_boxed(none())
                .push_boxed(text("a"))
        ),
        1
    );
    assert_eq!(
        count(
            &mut Column::<El<'_>>::new()
                .push_boxed(none())
                .push_boxed(text("a"))
        ),
        1
    );
    assert_eq!(
        count(
            &mut Stack::<El<'_>>::new()
                .push_boxed(none())
                .push_boxed(text("a"))
        ),
        1
    );
    assert_eq!(
        count(&mut Row::<El<'_>>::new().extend_boxed([none(), none()])),
        0
    );
}

/// Boxed children are added in order by `extend_boxed`, in every container.
#[test]
fn extend_boxed_adds_all_children() {
    let texts = || ["a", "b", "c"].map(text);

    assert_eq!(count(&mut Row::<El<'_>>::new().extend_boxed(texts())), 3);
    assert_eq!(count(&mut Column::<El<'_>>::new().extend_boxed(texts())), 3);
    assert_eq!(count(&mut Stack::<El<'_>>::new().extend_boxed(texts())), 3);
    assert_eq!(count(&mut Grid::<El<'_>>::new().extend_boxed(texts())), 3);
}

/// Mixed types in a loop, no `.boxed()` needed.
#[test]
fn loop_building_column_from_three_widget_types() {
    let mut column = Column::<El<'_>>::new();

    for i in 0..6 {
        column = match i % 3 {
            0 => column.push_boxed(text("t")),
            1 => column.push_boxed(button("b")),
            _ => column.push_boxed(space()),
        };
    }

    assert_eq!(count(&mut column), 6);
}

/// `Stack` and `Grid` `push_boxed` keep their children.
#[test]
fn stack_and_grid_push_boxed() {
    assert_eq!(
        count(
            &mut Stack::<El<'_>>::new()
                .push_boxed(text("a"))
                .push_boxed(space())
        ),
        2
    );
    assert_eq!(
        count(
            &mut Grid::<El<'_>>::new()
                .push_boxed(text("a"))
                .push_boxed(button("b"))
        ),
        2
    );
}

/// Keyed columns box children and skip `None`.
#[test]
fn keyed_column_push_boxed() {
    let mut column = keyed::Column::<u32, El<'_>>::new()
        .push_boxed(0, text("a"))
        .push_boxed(1, button("b"))
        .push_boxed(2, None::<iced_widget::Text<'static>>)
        .extend_boxed([(3, text("c")), (4, text("d"))]);

    assert_eq!(count(&mut column), 4);
}

/// `None` is skipped by `Grid::push_boxed` and by `keyed::Column`'s
/// `extend_boxed`.
#[test]
fn none_children_skipped_in_grid_and_keyed() {
    let none = || None::<iced_widget::Text<'static>>;

    assert_eq!(
        count(
            &mut Grid::<El<'_>>::new()
                .push_boxed(none())
                .push_boxed(text("a"))
        ),
        1
    );
    assert_eq!(
        count(&mut Grid::<El<'_>>::new().extend_boxed([none(), none()])),
        0
    );

    let mut column = keyed::Column::<u32, El<'_>>::new()
        .extend_boxed([(0, none()), (1, none())])
        .push_boxed(2, text("a"));

    assert_eq!(count(&mut column), 1);
}

/// `push_under_boxed` adds under and skips `None` without shifting the base
/// layer.
#[test]
fn stack_push_under_boxed() {
    let mut stack = Stack::<El<'_>>::new()
        .push_boxed(text("top"))
        .push_under_boxed(space())
        .push_under_boxed(None::<iced_widget::Text<'static>>);

    assert_eq!(count(&mut stack), 2);
}
