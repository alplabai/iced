//! Tests for the `keyed::Column` widget.
//!
//! Regression tests for fork issue #8: widget state must follow the child's
//! key when the children are reordered, inserted or removed.
use iced_test::Simulator;
use iced_widget::Theme;
use iced_widget::core::layout;
use iced_widget::core::mouse;
use iced_widget::core::renderer;
use iced_widget::core::widget::{Meta, Tree, tree};
use iced_widget::core::{Layout, Length, Never, Rectangle, Size, Widget};
use iced_widget::keyed;
use iced_widget::{Component, Renderer, button, component, keyed_column, text};

/// A stateful leaf widget. Its state records the id it was created with and
/// is never overwritten by `diff`, so it reveals which tree a widget was
/// matched with.
struct Probe(u32);

struct ProbeState(u32);

impl Meta for Probe {}

impl Widget<Never, Theme, ()> for Probe {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ProbeState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ProbeState(self.0))
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(10.0), Length::Fixed(10.0))
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &(), _limits: &layout::Limits) {}

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

fn column(keys: &[char]) -> keyed::Column<char, Probe> {
    keyed::Column::from_vecs(
        keys.to_vec(),
        keys.iter().map(|key| Probe(*key as u32)).collect(),
    )
}

/// Returns the id stored in each child tree, in order.
fn states(tree: &Tree) -> Vec<u32> {
    tree.children
        .iter()
        .map(|child| child.state.downcast_ref::<ProbeState>().0)
        .collect()
}

fn diff(column: &mut keyed::Column<char, Probe>, tree: &mut Tree) {
    <keyed::Column<char, Probe> as Widget<Never, Theme, ()>>::diff(column, tree);
}

fn build(column: &mut keyed::Column<char, Probe>) -> Tree {
    let mut tree = Tree::new::<Never, Theme, ()>(&*column);
    diff(column, &mut tree);
    tree
}

fn id(key: char) -> u32 {
    key as u32
}

#[test]
fn state_follows_key_on_reorder() {
    let mut first = column(&['a', 'b', 'c']);
    let mut tree = build(&mut first);

    assert_eq!(states(&tree), vec![id('a'), id('b'), id('c')]);

    // A positional diff would keep [a, b, c] here, handing key `c` the state
    // of key `a`.
    let mut reordered = column(&['c', 'a', 'b']);
    diff(&mut reordered, &mut tree);

    assert_eq!(states(&tree), vec![id('c'), id('a'), id('b')]);
}

#[test]
fn state_follows_key_on_insert_and_remove() {
    let mut first = column(&['a', 'b', 'c']);
    let mut tree = build(&mut first);

    // Remove the first key: `b` and `c` must keep their own state.
    let mut removed = column(&['b', 'c']);
    diff(&mut removed, &mut tree);
    assert_eq!(states(&tree), vec![id('b'), id('c')]);

    // Insert a new key in the middle: existing keys keep their state and the
    // new key gets fresh state.
    let mut inserted = column(&['b', 'x', 'c']);
    diff(&mut inserted, &mut tree);
    assert_eq!(states(&tree), vec![id('b'), id('x'), id('c')]);
}

#[test]
fn unchanged_keys_keep_state() {
    let mut first = column(&['a', 'b']);
    let mut tree = build(&mut first);

    let mut same = column(&['a', 'b']);
    diff(&mut same, &mut tree);

    assert_eq!(states(&tree), vec![id('a'), id('b')]);
}

#[test]
#[should_panic(expected = "exactly one key per child")]
fn from_vecs_panics_on_length_mismatch() {
    let _ = keyed::Column::<char, Probe>::from_vecs(vec!['a', 'b'], vec![Probe(1)]);
}

/// A button that counts how many times it has been pressed in its own state.
struct Counter(&'static str);

impl<'a> Component<'a, ()> for Counter {
    type State = u32;
    type Event = ();

    fn update(&self, count: &mut u32, _event: (), _renderer: &Renderer) -> Option<()> {
        *count += 1;
        None
    }

    fn view(&self, count: &u32) -> impl Widget<(), Theme, Renderer> + 'a {
        button(text(format!("{} {count}", self.0))).on_press(())
    }
}

fn counters(keys: &[&'static str]) -> impl Widget<(), Theme, Renderer> + 'static {
    keyed_column(keys.iter().map(|&key| (key, component(Counter(key)))))
}

/// Presses the counter of `key` once, rebuilds the column with `after` and
/// checks that the state stayed with `key`.
fn assert_state_follows_key(before: &[&'static str], after: &[&'static str], key: &str) {
    let mut simulator = Simulator::new(counters(before));

    let _ = simulator.click(format!("{key} 0").as_str()).unwrap();
    assert!(simulator.find(format!("{key} 1").as_str()).is_ok());

    let mut simulator = simulator.rebuild(counters(after));

    for other in after {
        let expected = if *other == key { 1 } else { 0 };

        assert!(
            simulator
                .find(format!("{other} {expected}").as_str())
                .is_ok(),
            "{other} should have a count of {expected} after rebuilding with {after:?}"
        );
    }
}

#[test]
fn state_is_kept_when_pushing() {
    assert_state_follows_key(&["a"], &["a", "b"], "a");
    assert_state_follows_key(&["a", "b"], &["a", "b", "c"], "b");
}

#[test]
fn state_is_kept_when_inserting_in_the_middle() {
    assert_state_follows_key(&["a", "c"], &["a", "b", "c"], "c");
    assert_state_follows_key(&["a", "c"], &["a", "b", "c"], "a");
}

#[test]
fn state_is_kept_when_inserting_at_the_start() {
    assert_state_follows_key(&["b"], &["a", "b"], "b");
}
