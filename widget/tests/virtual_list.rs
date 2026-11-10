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

use iced_widget::core::{Event, Point, window};
use iced_widget::core::{image, shell};

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

impl<R: iced_widget::core::Renderer> Widget<Never, Theme, R> for Probe {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ProbeState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ProbeState(self.0))
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &R, limits: &layout::Limits) {
        tree.size = limits.max;
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut R,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                ..renderer::Quad::default()
            },
            iced_widget::core::Color::BLACK,
        );
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

/// A renderer that records the clip of every layer and the quads it is given.
#[derive(Default)]
struct Recorder {
    layers: Vec<Rectangle>,
    quads: usize,
}

impl renderer::Renderer for Recorder {
    fn start_layer(&mut self, bounds: Rectangle) {
        self.layers.push(bounds);
    }

    fn end_layer(&mut self) {}

    fn start_transformation(&mut self, _transformation: iced_widget::core::Transformation) {}

    fn end_transformation(&mut self) {}

    fn fill_quad(
        &mut self,
        _quad: renderer::Quad,
        _background: impl Into<iced_widget::core::Background>,
    ) {
        self.quads += 1;
    }

    fn allocate_image(
        &self,
        _handle: &image::Handle,
        _callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    ) {
    }

    fn hint(&mut self, _scale: renderer::Scale) {}

    fn scale(&self) -> Option<renderer::Scale> {
        None
    }

    fn reset(&mut self, _new_bounds: Rectangle) {}

    fn settings(&self) -> renderer::Settings {
        renderer::Settings::default()
    }
}

#[test]
fn draw_clips_to_the_visible_part_of_the_list() {
    let mut list = VirtualList::new(100, 10.0, |i| Probe(i as u64));
    let mut tree = Tree::new::<Never, Theme, Recorder>(&list);

    Widget::<Never, Theme, Recorder>::layout(
        &mut list,
        &mut tree,
        &Recorder::default(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 100.0)),
    );

    // The parent only shows the top half of the list.
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(100.0, 50.0));
    let mut recorder = Recorder::default();

    Widget::<Never, Theme, Recorder>::draw(
        &list,
        &tree,
        &mut recorder,
        &Theme::Light,
        &renderer::Style::default(),
        Layout::new(tree.size),
        mouse::Cursor::Unavailable,
        &viewport,
    );

    // The rows must not be able to paint outside of the parent clip.
    assert_eq!(recorder.layers, vec![viewport]);
    assert_eq!(recorder.quads, 5);
}

#[test]
fn unbounded_height_does_not_build_every_row() {
    let built = Cell::new(0);

    let mut list = VirtualList::new(100_000, 10.0, |i| {
        built.set(built.get() + 1);
        Probe(i as u64)
    });
    let mut tree = Tree::new::<Never, Theme, ()>(&list);

    list.layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, f32::INFINITY)),
    );

    assert!(tree.size.height.is_finite());
    assert!(built.get() < 1_000, "built {} rows", built.get());
}

#[test]
fn flagged_infinite_height_does_not_build_every_row() {
    let built = Cell::new(0);

    let mut list = VirtualList::new(100_000, 10.0, |i| {
        built.set(built.get() + 1);
        Probe(i as u64)
    });
    let mut tree = Tree::new::<Never, Theme, ()>(&list);

    // What a vertical `scrollable` hands to its content: a finite viewport
    // height, flagged as infinite
    list.layout(
        &mut tree,
        &(),
        &layout::Limits::with_flags(
            Size::ZERO,
            Size::new(100.0, 95.0),
            Size::new(false, false),
            Size::new(false, true),
        ),
    );

    assert!(built.get() < 1_000, "built {} rows", built.get());
    assert_eq!(tree.size.height, 1_000_000.0);
}

#[test]
fn pixel_scrolling_moves_rows_without_a_relayout() {
    let mut list = VirtualList::new(100, 10.0, |i| Probe(i as u64));
    let mut tree = Tree::new::<Never, Theme, ()>(&list);

    list.layout(
        &mut tree,
        &(),
        &layout::Limits::new(Size::ZERO, Size::new(100.0, 95.0)),
    );

    let scroll = |list: &mut VirtualList<'_, Probe>, tree: &mut Tree, y: f32| {
        let mut messages = shell::Bus::new();
        let mut shell = shell::Shell::new(&window::Headless, shell::Waker::noop(), &mut messages);

        list.update(
            tree,
            &Event::Mouse(mouse::Event::WheelScrolled {
                delta: ScrollDelta::Pixels { x: 0.0, y },
            }),
            Layout::new(tree.size),
            mouse::Cursor::Available(Point::new(5.0, 5.0)),
            &(),
            &mut shell,
            &Rectangle::with_size(Size::new(100.0, 95.0)),
        );

        shell.invalidation()
    };

    // Still the same rows: they are shifted in place.
    assert_eq!(
        scroll(&mut list, &mut tree, -3.0),
        shell::Invalidation::None
    );
    assert_eq!(tree.children[0].translation.y, -3.0);
    assert_eq!(tree.children[1].translation.y, 7.0);

    // A new row becomes visible: a relayout is needed.
    assert!(matches!(
        scroll(&mut list, &mut tree, -4.0),
        shell::Invalidation::Layout(_)
    ));
}

// -- Inside a `scrollable` ---------------------------------------------------

mod windowed {
    use super::*;

    use iced_widget::Renderer;
    use iced_widget::core::overlay;
    use iced_widget::core::widget::operation::scrollable::{self, RelativeOffset};
    use iced_widget::core::{Vector, window};
    use iced_widget::scrollable::{Anchor, Scroll, Scrollable};
    use iced_widget::{Column, Text, space};

    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    const FRAME: Duration = Duration::from_millis(16);
    const ID: Id = Id::new("scrollable");

    type Rows = VirtualList<'static, Text<'static, Theme>>;

    fn rows(len: usize, row_height: f32, built: Rc<Cell<usize>>) -> Rows {
        VirtualList::new(len, row_height, move |i| {
            built.set(built.get() + 1);
            text::<Theme>(format!("Row {i}"))
        })
    }

    /// A `scrollable` of `len` rows of 20 px, `viewport` px tall.
    fn windowed(len: usize, viewport: u32) -> Scrollable<'static, Scroll, Rows, Theme> {
        windowed_counted(len, viewport, Rc::new(Cell::new(0)))
    }

    fn windowed_counted(
        len: usize,
        viewport: u32,
        built: Rc<Cell<usize>>,
    ) -> Scrollable<'static, Scroll, Rows, Theme> {
        Scrollable::new(rows(len, 20.0, built))
            .id(ID)
            .width(Length::Fill)
            .height(viewport)
            .on_scroll(Some)
    }

    fn step_frames<M: 'static>(
        simulator: &mut Simulator<'_, M, Theme, Renderer>,
        instant: &mut Instant,
        frames: usize,
    ) {
        for _ in 0..frames {
            *instant += FRAME;
            std::thread::sleep(FRAME);

            let _ = simulator.simulate([Event::Window(window::Event::RedrawRequested(*instant))]);
        }
    }

    fn has<M: 'static>(simulator: &mut Simulator<'_, M, Theme, Renderer>, row: usize) -> bool {
        simulator.find(format!("Row {row}").as_str()).is_ok()
    }

    fn last_offset(simulator: &mut Simulator<'_, Scroll, Theme, Renderer>) -> Option<f32> {
        simulator
            .drain()
            .last()
            .map(|scroll| scroll.viewport.absolute_offset().y)
    }

    fn left_press() -> Event {
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
    }

    fn left_release() -> Event {
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
    }

    fn move_to(simulator: &mut Simulator<'_, Scroll, Theme, Renderer>, x: f32, y: f32) {
        simulator.point_at(Point::new(x, y));
        let _ = simulator.simulate([Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(x, y),
        })]);
    }

    #[test]
    fn infinite_flag_reports_full_content_height() {
        let built = Cell::new(0);

        let mut list = VirtualList::new(100_000, 10.0, |i| {
            built.set(built.get() + 1);
            Probe(i as u64)
        });
        let mut tree = Tree::new::<Never, Theme, ()>(&list);

        list.layout(
            &mut tree,
            &(),
            &layout::Limits::with_flags(
                Size::ZERO,
                Size::new(100.0, 95.0),
                Size::new(false, false),
                Size::new(false, true),
            ),
        );

        assert_eq!(tree.size.height, 1_000_000.0);
        assert_eq!(built.get(), 12);
    }

    #[test]
    fn scrollable_wheel_rebuilds_rows() {
        let built = Rc::new(Cell::new(0));
        let mut simulator =
            Simulator::new(windowed_counted(100_000, 100, built.clone()).smooth_scroll(false));
        simulator.point_at(Point::new(5.0, 5.0));

        assert!(has(&mut simulator, 0));

        // 40 lines of 120 px: row 240.
        let _ = simulator.scroll(ScrollDelta::Lines { x: 0.0, y: -40.0 });

        built.set(0);
        let mut instant = Instant::now();
        step_frames(&mut simulator, &mut instant, 1);

        // The scrollable, not the list, took the wheel.
        assert_eq!(last_offset(&mut simulator), Some(4_800.0));
        assert!(has(&mut simulator, 240));
        assert!(!has(&mut simulator, 0));
        assert!(built.get() <= 30, "built {} rows", built.get());
    }

    #[test]
    fn smooth_scroll_rebuilds_rows_each_frame() {
        let mut simulator = Simulator::new(windowed(100_000, 100));
        simulator.point_at(Point::new(5.0, 5.0));

        let _ = simulator.scroll(ScrollDelta::Lines { x: 0.0, y: -10.0 });

        let mut instant = Instant::now();
        let mut seen = 0;

        for _ in 0..60 {
            step_frames(&mut simulator, &mut instant, 1);

            if let Some(offset) = last_offset(&mut simulator) {
                let (first, last) = ((offset / 20.0) as usize, ((offset + 99.0) / 20.0) as usize);

                assert!(has(&mut simulator, first), "offset {offset}: row {first}");
                assert!(has(&mut simulator, last), "offset {offset}: row {last}");
                seen += 1;
            }
        }

        assert!(seen > 3, "the scroll was not gradual");
        assert!(has(&mut simulator, 60));
    }

    #[test]
    fn scroller_drag_to_end_builds_last_rows() {
        let mut simulator = Simulator::new(windowed(100_000, 200));

        let mut instant = Instant::now();
        step_frames(&mut simulator, &mut instant, 1);

        simulator.point_at(Point::new(1019.0, 6.0));
        let _ = simulator.simulate([left_press()]);
        move_to(&mut simulator, 1019.0, 199.0);
        let _ = simulator.simulate([left_release()]);

        step_frames(&mut simulator, &mut instant, 2);

        assert!(has(&mut simulator, 99_999));
        assert!(!has(&mut simulator, 0));
    }

    #[test]
    fn rail_press_pages_and_rebuilds() {
        let mut simulator = Simulator::new(windowed(100_000, 200));

        let mut instant = Instant::now();
        step_frames(&mut simulator, &mut instant, 1);

        simulator.point_at(Point::new(1019.0, 150.0));
        let _ = simulator.simulate([left_press()]);
        step_frames(&mut simulator, &mut instant, 10);
        let _ = simulator.simulate([left_release()]);
        step_frames(&mut simulator, &mut instant, 2);

        let offset = last_offset(&mut simulator).expect("the rail press scrolled");
        assert!(offset > 100.0, "offset {offset}");

        assert!(has(&mut simulator, (offset / 20.0) as usize));
        assert!(!has(&mut simulator, 0));
    }

    #[test]
    fn reveal_row_via_scrollable_id() {
        let mut simulator = Simulator::new(windowed(1_000, 100));

        simulator.operate(&mut list::reveal_row::<()>(ID, 500, 20.0));

        let mut instant = Instant::now();
        step_frames(&mut simulator, &mut instant, 2);

        assert!(has(&mut simulator, 500));
        assert!(!has(&mut simulator, 0));
    }

    #[test]
    fn anchor_bottom_shows_last_rows_after_first_frame() {
        let mut simulator = Simulator::new(windowed(1_000, 100).anchor_y(Anchor::End));

        let mut instant = Instant::now();
        step_frames(&mut simulator, &mut instant, 1);

        assert!(has(&mut simulator, 999));
        assert!(!has(&mut simulator, 0));
    }

    /// A [`Probe`] that records the id of its state whenever it is drawn.
    struct Tracked(u64);

    thread_local! {
        static DRAWN: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
    }

    impl Meta for Tracked {}

    impl<R: iced_widget::core::Renderer> Widget<Never, Theme, R> for Tracked {
        fn tag(&self) -> tree::Tag {
            tree::Tag::of::<ProbeState>()
        }

        fn state(&self) -> tree::State {
            tree::State::new(ProbeState(self.0))
        }

        fn size(&self) -> Size<Length> {
            Size::new(Length::Fill, Length::Fill)
        }

        fn layout(&mut self, tree: &mut Tree, _renderer: &R, limits: &layout::Limits) {
            tree.size = limits.max;
        }

        fn draw(
            &self,
            tree: &Tree,
            _renderer: &mut R,
            _theme: &Theme,
            _style: &renderer::Style,
            _layout: Layout,
            _cursor: mouse::Cursor,
            _viewport: &Rectangle,
        ) {
            let id = tree.state.downcast_ref::<ProbeState>().0;
            DRAWN.with(|drawn| drawn.borrow_mut().push(id));
        }
    }

    fn tracked(offset: u64) -> Scrollable<'static, Never, VirtualList<'static, Tracked>, Theme> {
        Scrollable::new(
            VirtualList::new(100, 10.0, move |i| Tracked(offset + i as u64)).overscan(0),
        )
        .id(ID)
        .width(Length::Fill)
        .height(100)
    }

    #[test]
    fn state_follows_key_when_scrollable_scrolls() {
        let mut simulator = Simulator::new(tracked(0));

        let mut instant = Instant::now();
        step_frames(&mut simulator, &mut instant, 1);

        // A fresh state would be visible: every row is now built with a
        // different id.
        let mut simulator = simulator.rebuild(tracked(1_000));

        simulator.operate(&mut scrollable::scroll_by::<()>(
            ID,
            AbsoluteOffset { x: 0.0, y: 50.0 },
            Animation::Instant,
        ));
        step_frames(&mut simulator, &mut instant, 2);

        DRAWN.with(|drawn| drawn.borrow_mut().clear());
        simulator.draw(&Theme::Light);

        assert_eq!(
            DRAWN.with(|drawn| drawn.borrow().clone()),
            vec![5, 6, 7, 8, 9, 1_010, 1_011, 1_012, 1_013, 1_014]
        );
    }

    /// A [`Probe`] with an overlay.
    struct WithOverlay;

    struct Marker;

    impl overlay::Overlay<Never, Theme, ()> for Marker {
        fn draw(&self, _: &mut (), _: &Theme, _: &renderer::Style, _: mouse::Cursor) {}
    }

    impl Meta for WithOverlay {}

    impl Widget<Never, Theme, ()> for WithOverlay {
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

        fn overlay<'a>(
            &'a mut self,
            _tree: &'a mut Tree,
            _layout: Layout,
            _renderer: &(),
            _viewport: &Rectangle,
            _translation: Vector,
            _window: Size,
        ) -> Vec<overlay::Element<'a, Never, Theme, ()>> {
            vec![overlay::Element::new(Box::new(Marker))]
        }
    }

    fn flagged(height: f32) -> layout::Limits {
        layout::Limits::with_flags(
            Size::ZERO,
            Size::new(100.0, height),
            Size::new(false, false),
            Size::new(false, true),
        )
    }

    /// Sends a neutral event to the list, as a scrollable showing `viewport`
    /// of it would, and returns the invalidation.
    fn update_in<W>(
        list: &mut VirtualList<'_, W>,
        tree: &mut Tree,
        viewport: Rectangle,
    ) -> shell::Invalidation
    where
        W: Widget<Never, Theme, ()>,
    {
        let mut messages = shell::Bus::new();
        let mut shell = shell::Shell::new(&window::Headless, shell::Waker::noop(), &mut messages);

        list.update(
            tree,
            &Event::Mouse(mouse::Event::CursorMoved {
                position: Point::ORIGIN,
            }),
            Layout::new(tree.size),
            mouse::Cursor::Unavailable,
            &(),
            &mut shell,
            &viewport,
        );

        shell.invalidation()
    }

    #[test]
    fn overscan_rows_produce_no_overlay_inside_scrollable() {
        let mut list = VirtualList::new(100, 10.0, |_| WithOverlay);
        let mut tree = Tree::new::<Never, Theme, ()>(&list);

        list.layout(&mut tree, &(), &flagged(100.0));

        let viewport = Rectangle::new(Point::ORIGIN, Size::new(100.0, 45.0));
        let _ = update_in(&mut list, &mut tree, viewport);
        list.layout(&mut tree, &(), &flagged(100.0));

        let layout = Layout::new(tree.size);
        let overlays = list
            .overlay(
                &mut tree,
                layout,
                &(),
                &viewport,
                Vector::ZERO,
                Size::new(100.0, 100.0),
            )
            .len();

        // Rows 0..5 are visible; rows 5 and 6 are overscan.
        assert_eq!(overlays, 5);
    }

    #[test]
    fn list_below_header_windows_from_its_own_top() {
        let content = Column::with_children([
            Widget::<Scroll, Theme, Renderer>::_boxed(space().height(200)),
            Widget::<Scroll, Theme, Renderer>::_boxed(rows(100_000, 20.0, Rc::new(Cell::new(0)))),
        ]);

        let mut simulator = Simulator::new(
            Scrollable::new(content)
                .id(ID)
                .width(Length::Fill)
                .height(300)
                .on_scroll(Some),
        );

        simulator.operate(&mut scrollable::scroll_to::<()>(
            ID,
            AbsoluteOffset {
                x: Some(0.0),
                y: Some(1_000.0),
            },
            Animation::Instant,
        ));

        let mut instant = Instant::now();
        step_frames(&mut simulator, &mut instant, 2);

        // The list starts 200 px down: 800 px of it are above the viewport.
        assert!(has(&mut simulator, 40));
        assert!(has(&mut simulator, 54));
        assert!(!has(&mut simulator, 0));
    }

    #[test]
    fn len_shrink_while_scrolled_deep_shows_tail() {
        let mut simulator = Simulator::new(windowed(100_000, 100));

        simulator.operate(&mut scrollable::snap_to::<()>(
            ID,
            RelativeOffset {
                x: None,
                y: Some(1.0),
            },
            Animation::Instant,
        ));

        let mut instant = Instant::now();
        step_frames(&mut simulator, &mut instant, 2);
        assert!(has(&mut simulator, 99_999));

        let mut simulator = simulator.rebuild(windowed(50_000, 100));
        step_frames(&mut simulator, &mut instant, 1);

        assert!(has(&mut simulator, 49_999));
        assert!(!has(&mut simulator, 0));
    }

    #[test]
    fn sub_row_scroll_inside_scrollable_needs_no_relayout() {
        let mut list = VirtualList::new(100, 10.0, |i| Probe(i as u64));
        let mut tree = Tree::new::<Never, Theme, ()>(&list);

        list.layout(&mut tree, &(), &flagged(95.0));

        let at = |y: f32| Rectangle::new(Point::new(0.0, y), Size::new(100.0, 95.0));

        assert_eq!(
            update_in(&mut list, &mut tree, at(0.0)),
            shell::Invalidation::None
        );
        assert_eq!(
            update_in(&mut list, &mut tree, at(3.0)),
            shell::Invalidation::None
        );

        // A new row comes into range.
        assert!(matches!(
            update_in(&mut list, &mut tree, at(7.0)),
            shell::Invalidation::Layout(_)
        ));
    }

    #[test]
    fn deep_rows_are_positioned_exactly() {
        let mut list = VirtualList::new(100_000, 24.0, |i| Probe(i as u64));
        let mut tree = Tree::new::<Never, Theme, ()>(&list);

        list.layout(&mut tree, &(), &flagged(200.0));

        let bottom = Rectangle::new(
            Point::new(0.0, 2_400_000.0 - 200.0),
            Size::new(100.0, 200.0),
        );
        assert!(matches!(
            update_in(&mut list, &mut tree, bottom),
            shell::Invalidation::Layout(_)
        ));

        list.layout(&mut tree, &(), &flagged(200.0));

        assert!(tree.children.len() <= 14);
        assert_eq!(tree.children.last().unwrap().translation.y, 99_999.0 * 24.0);
    }
}
