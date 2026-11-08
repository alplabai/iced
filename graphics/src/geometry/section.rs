//! Keep the draw order of the contents of a [`Frame`].
//!
//! A [`Layer`] of a renderer draws its contents grouped by kind in a fixed
//! order: shapes first, then custom primitives, then images, and text last. A [`Frame`], however,
//! lets users interleave these kinds freely.
//!
//! To preserve the order in which things were drawn, a [`Frame`] splits its
//! contents into sections. Each section only contains kinds in the order a
//! [`Layer`] draws them, so it can be rendered in a single [`Layer`]. A new
//! section only starts when the kind of what is being drawn goes "backwards"
//! (e.g. a shape drawn after an image). Frames that only draw shapes, or only
//! images, or shapes and then images, never produce more than one section.
//!
//! [`Frame`]: crate::geometry::Frame
//! [`Layer`]: crate::layer::Layer
use std::mem;

/// A kind of content that can be drawn in a [`Frame`].
///
/// The variants are ordered like a [`Layer`] draws them.
///
/// [`Frame`]: crate::geometry::Frame
/// [`Layer`]: crate::layer::Layer
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// Filled or stroked shapes (meshes or vector primitives).
    Shape,
    /// Custom renderer-specific primitives, like shaders.
    Primitive,
    /// Raster or vector images.
    Image,
    /// Text.
    Text,
}

/// A list of sections of content that keeps their draw order.
#[derive(Debug, Clone)]
pub struct Sections<T> {
    finished: Vec<T>,
    current: T,
    last: Option<Kind>,
}

impl<T: Default> Sections<T> {
    /// Creates a new [`Sections`] with a single empty section.
    pub fn new() -> Self {
        Self {
            finished: Vec::new(),
            current: T::default(),
            last: None,
        }
    }

    /// Announces that something of the given [`Kind`] is about to be added,
    /// and returns the section it must be added to.
    ///
    /// A new section is started if the [`Kind`] could not be drawn after
    /// the previous content of the current section.
    pub fn begin(&mut self, kind: Kind) -> &mut T {
        self.begin_with(kind, |_| {})
    }

    /// Like [`begin`](Self::begin), but `seal` is called with the current
    /// section right before it is finished, in case a new section is started.
    ///
    /// This is useful for sections that buffer some of their contents.
    pub fn begin_with(&mut self, kind: Kind, seal: impl FnOnce(&mut T)) -> &mut T {
        if self.last.is_some_and(|last| kind < last) {
            seal(&mut self.current);

            self.finished.push(mem::take(&mut self.current));
        }

        self.last = Some(kind);

        &mut self.current
    }

    /// Returns the current section without announcing any content.
    pub fn current_mut(&mut self) -> &mut T {
        &mut self.current
    }

    /// Returns the amount of sections, including the current one.
    pub fn len(&self) -> usize {
        self.finished.len() + 1
    }

    /// Returns true if no section has been finished yet and no content has
    /// been announced.
    pub fn is_empty(&self) -> bool {
        self.finished.is_empty() && self.last.is_none()
    }

    /// Finishes the current section and returns all the sections in draw
    /// order. There is always at least one section.
    ///
    /// `seal` is called with the current section before it is finished.
    pub fn finish(mut self, seal: impl FnOnce(&mut T)) -> Vec<T> {
        seal(&mut self.current);

        self.finished.push(self.current);
        self.finished
    }
}

impl<T: Default> Default for Sections<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, PartialEq)]
    struct Section(Vec<Kind>);

    fn draw(sections: &mut Sections<Section>, kinds: &[Kind]) {
        for &kind in kinds {
            sections.begin(kind).0.push(kind);
        }
    }

    use Kind::{Image, Primitive, Shape, Text};

    #[test]
    fn empty_has_a_single_empty_section() {
        let sections = Sections::<Section>::new();

        assert!(sections.is_empty());
        assert_eq!(sections.len(), 1);
        assert_eq!(sections.finish(|_| {}), vec![Section(vec![])]);
    }

    #[test]
    fn shapes_only_is_a_single_section() {
        let mut sections = Sections::new();
        draw(&mut sections, &[Shape, Shape, Shape]);

        assert_eq!(
            sections.finish(|_| {}),
            vec![Section(vec![Shape, Shape, Shape])]
        );
    }

    #[test]
    fn images_only_is_a_single_section() {
        let mut sections = Sections::new();
        draw(&mut sections, &[Image, Image]);

        assert_eq!(sections.finish(|_| {}), vec![Section(vec![Image, Image])]);
    }

    #[test]
    fn layer_order_is_a_single_section() {
        let mut sections = Sections::new();
        draw(&mut sections, &[Shape, Shape, Image, Text, Text]);

        assert_eq!(
            sections.finish(|_| {}),
            vec![Section(vec![Shape, Shape, Image, Text, Text])]
        );
    }

    #[test]
    fn primitive_sits_between_shapes_and_images() {
        let mut sections = Sections::new();
        draw(&mut sections, &[Shape, Primitive, Image, Text]);

        assert_eq!(
            sections.finish(|_| {}),
            vec![Section(vec![Shape, Primitive, Image, Text])]
        );
    }

    #[test]
    fn shape_after_primitive_starts_a_section() {
        let mut sections = Sections::new();
        draw(&mut sections, &[Shape, Primitive, Shape, Primitive]);

        assert_eq!(
            sections.finish(|_| {}),
            vec![
                Section(vec![Shape, Primitive]),
                Section(vec![Shape, Primitive])
            ]
        );
    }

    #[test]
    fn primitive_after_image_starts_a_section() {
        let mut sections = Sections::new();
        draw(&mut sections, &[Image, Primitive]);

        assert_eq!(
            sections.finish(|_| {}),
            vec![Section(vec![Image]), Section(vec![Primitive])]
        );
    }

    #[test]
    fn shape_after_image_starts_a_section() {
        let mut sections = Sections::new();
        draw(&mut sections, &[Shape, Image, Shape]);

        assert_eq!(sections.len(), 2);
        assert_eq!(
            sections.finish(|_| {}),
            vec![Section(vec![Shape, Image]), Section(vec![Shape])]
        );
    }

    #[test]
    fn image_after_text_starts_a_section() {
        let mut sections = Sections::new();
        draw(&mut sections, &[Text, Image]);

        assert_eq!(
            sections.finish(|_| {}),
            vec![Section(vec![Text]), Section(vec![Image])]
        );
    }

    #[test]
    fn alternating_kinds_split_every_time_order_is_lost() {
        let mut sections = Sections::new();
        draw(&mut sections, &[Image, Shape, Image, Shape, Text, Shape]);

        assert_eq!(
            sections.finish(|_| {}),
            vec![
                Section(vec![Image]),
                Section(vec![Shape, Image]),
                Section(vec![Shape, Text]),
                Section(vec![Shape]),
            ]
        );
    }

    #[test]
    fn seal_is_called_on_every_finished_section() {
        let mut sections = Sections::<Section>::new();
        let mut sealed = 0;

        sections.begin_with(Image, |_| sealed += 1).0.push(Image);
        sections.begin_with(Shape, |_| sealed += 1).0.push(Shape);
        sections.begin_with(Shape, |_| sealed += 1).0.push(Shape);

        assert_eq!(sealed, 1);

        let sections = sections.finish(|section| {
            sealed += 1;
            section.0.push(Text);
        });

        assert_eq!(sealed, 2);
        assert_eq!(
            sections,
            vec![Section(vec![Image]), Section(vec![Shape, Shape, Text])]
        );
    }
}
