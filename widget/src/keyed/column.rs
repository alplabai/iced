//! Keyed columns distribute content vertically while keeping continuity.
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::widget::tree::{self, Tree};
use crate::core::widget::{Meta, Operation};
use crate::core::{
    Alignment, Event, Layout, Length, Padding, Pixels, Rectangle, Shell, Size, Vector, Widget,
};

/// A container that distributes its contents vertically while keeping continuity.
///
/// # Example
/// ```no_run
/// # mod iced { pub mod widget { pub use iced_widget::*; } pub use iced_widget::Renderer; pub use iced_widget::core::*; }
/// # use iced::widget::Widget;
/// # pub type State = ();
/// use iced::widget::{keyed_column, text};
///
/// enum Message {
///     // ...
/// }
///
/// fn view(state: &State) -> impl Widget<Message> {
///     keyed_column((0..=100).map(|i| {
///         (i, text!("Item {i}"))
///     }))
/// }
/// ```
pub struct Column<Key, W>
where
    Key: Copy + PartialEq,
{
    spacing: f32,
    padding: Padding,
    width: Length,
    height: Length,
    align_items: Alignment,
    keys: Vec<Key>,
    children: Vec<W>,
}

impl<Key, W> Column<Key, W>
where
    Key: Copy + PartialEq,
{
    /// Creates an empty [`Column`].
    pub fn new() -> Self {
        Self::from_vecs(Vec::new(), Vec::new())
    }

    /// Creates a [`Column`] from already allocated [`Vec`]s.
    ///
    /// Keep in mind that the [`Column`] will not inspect the [`Vec`]s, which means
    /// it won't automatically adapt to the sizing strategy of its contents.
    ///
    /// If any of the children have a [`Length::Fill`] strategy, you will need to
    /// call [`Column::width`] or [`Column::height`] accordingly.
    ///
    /// # Panics
    /// Panics if `keys` and `children` have different lengths.
    pub fn from_vecs(keys: Vec<Key>, children: Vec<W>) -> Self {
        assert_eq!(
            keys.len(),
            children.len(),
            "keyed::Column::from_vecs requires exactly one key per child"
        );

        Self {
            spacing: 0.0,
            padding: Padding::ZERO,
            width: Length::Fit,
            height: Length::Fit,
            align_items: Alignment::Start,
            keys,
            children,
        }
    }

    /// Creates a [`Column`] with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_vecs(Vec::with_capacity(capacity), Vec::with_capacity(capacity))
    }

    /// Creates a [`Column`] with the given elements.
    pub fn with_children(children: impl IntoIterator<Item = (Key, W)>) -> Self
    where
        W: Meta,
    {
        let iterator = children.into_iter();

        Self::with_capacity(iterator.size_hint().0).extend(iterator)
    }

    /// Sets the vertical spacing _between_ elements.
    ///
    /// Custom margins per element do not exist in iced. You should use this
    /// method instead! While less flexible, it helps you keep spacing between
    /// elements consistent.
    pub fn spacing(mut self, amount: impl Into<Pixels>) -> Self {
        self.spacing = amount.into().0;
        self
    }

    /// Sets the [`Padding`] of the [`Column`].
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the width of the [`Column`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Column`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the horizontal alignment of the contents of the [`Column`] .
    pub fn align_items(mut self, align: Alignment) -> Self {
        self.align_items = align;
        self
    }

    /// Adds an element to the [`Column`].
    pub fn push(mut self, key: Key, child: W) -> Self
    where
        W: Meta,
    {
        if !child.is_void() {
            self.keys.push(key);
            self.children.push(child);
        }

        self
    }

    /// Adds an element to the [`Column`], if `Some`.
    pub fn push_maybe(self, key: Key, child: Option<W>) -> Self
    where
        W: Meta,
    {
        if let Some(child) = child {
            self.push(key, child)
        } else {
            self
        }
    }

    /// Extends the [`Column`] with the given children.
    pub fn extend(self, children: impl IntoIterator<Item = (Key, W)>) -> Self
    where
        W: Meta,
    {
        children
            .into_iter()
            .fold(self, |column, (key, child)| column.push(key, child))
    }
}

impl<'a, Key, Message, Theme, Renderer>
    Column<Key, crate::core::Element<'a, Message, Theme, Renderer>>
where
    Key: Copy + PartialEq,
{
    /// Adds any [`Widget`] to the [`Column`], boxing it into an
    /// [`Element`](crate::core::Element) first.
    ///
    /// This is a shorthand for `push(key, child._boxed())`, useful when the
    /// children have different types.
    ///
    /// ```
    /// # use iced_widget::core::Element;
    /// # use iced_widget::{button, text, keyed};
    /// let view: keyed::Column<u32, Element<'_, (), iced_widget::Theme, iced_widget::Renderer>> =
    ///     keyed::Column::new()
    ///         .push_boxed(0, text("Hello"))
    ///         .push_boxed(1, button("Click"));
    /// ```
    pub fn push_boxed(self, key: Key, child: impl Widget<Message, Theme, Renderer> + 'a) -> Self {
        self.push(key, child._boxed())
    }

    /// Extends the [`Column`] with the given children, boxing each of them
    /// into an [`Element`](crate::core::Element) first.
    pub fn extend_boxed<I>(self, children: impl IntoIterator<Item = (Key, I)>) -> Self
    where
        I: Widget<Message, Theme, Renderer> + 'a,
    {
        children
            .into_iter()
            .fold(self, |column, (key, child)| column.push_boxed(key, child))
    }
}

impl<Key, W> Default for Column<Key, W>
where
    Key: Copy + PartialEq,
{
    fn default() -> Self {
        Self::new()
    }
}

struct State<Key>
where
    Key: Copy + PartialEq,
{
    keys: Vec<Key>,
    cache: layout::flex::Cache,
}

impl<Key, W> Meta for Column<Key, W> where Key: Copy + PartialEq {}

impl<Key, W, Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Column<Key, W>
where
    Renderer: crate::core::Renderer,
    Key: Copy + PartialEq + 'static,
    W: Widget<Message, Theme, Renderer>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Key>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            keys: self.keys.clone(),
            cache: layout::flex::Cache::default(),
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        let Tree {
            state, children, ..
        } = tree;

        let state = state.downcast_mut::<State<Key>>();

        if state.keys != self.keys || children.len() != self.keys.len() {
            // Reconcile children by key, so that widget state follows its key
            // even if the children were reordered, inserted, or removed.
            let mut old: Vec<Option<Tree>> =
                std::mem::take(children).into_iter().map(Some).collect();

            *children = self
                .keys
                .iter()
                .zip(&self.children)
                .map(|(key, child)| {
                    let found = state
                        .keys
                        .iter()
                        .zip(old.iter())
                        .position(|(old_key, tree)| old_key == key && tree.is_some());

                    found
                        .and_then(|index| old[index].take())
                        .unwrap_or_else(|| Tree::new(child))
                })
                .collect();
        }

        for (child_state, child) in children.iter_mut().zip(self.children.iter_mut()) {
            Tree::diff(child_state, child);
        }

        if state.keys != self.keys {
            state.keys.clone_from(&self.keys);
        }

        if self.width.is_fit() || self.height.is_fit() {
            for child in &self.children {
                let size = child.size();

                self.width = self.width.cross(size.width);
                self.height = self.height.stack(size.height);
            }
        }
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        let state = tree.state.downcast_mut::<State<Key>>();

        tree.size = layout::flex::resolve(
            layout::flex::Axis::Vertical,
            renderer,
            limits,
            self.width,
            self.height,
            self.padding,
            self.spacing,
            self.align_items,
            &mut tree.children,
            &mut self.children,
            &mut state.cache,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            self.children
                .iter_mut()
                .zip(layout.iter_mut(&mut tree.children))
                .for_each(|(child, (layout, state))| {
                    child.operate(state, layout, viewport, renderer, operation);
                });
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        for (child, (layout, tree)) in self
            .children
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children))
        {
            child.update(tree, event, layout, cursor, renderer, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.children
            .iter()
            .zip(layout.iter(&tree.children))
            .map(|(child, (layout, tree))| {
                child.mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        for (child, (layout, state)) in self.children.iter().zip(layout.iter(&tree.children)) {
            child.draw(state, renderer, theme, style, layout, cursor, viewport);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut self.children,
            tree,
            layout,
            renderer,
            viewport,
            translation,
            window,
        )
    }
}
