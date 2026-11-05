//! Operate on widgets that can be scrolled.
use crate::widget::operation::Animation;
use crate::widget::{Id, Operation};
use crate::{Rectangle, Size, Vector};

/// The internal state of a widget that can be scrolled.
pub trait Scrollable {
    /// Snaps the scroll of the widget to the given `percentage` along the horizontal & vertical axis.
    fn snap_to(
        &mut self,
        offset: RelativeOffset<Option<f32>>,
        animation: Animation,
        bounds: Rectangle,
        content: Size,
    );

    /// Scroll the widget to the given [`AbsoluteOffset`] along the horizontal & vertical axis.
    fn scroll_to(
        &mut self,
        offset: AbsoluteOffset<Option<f32>>,
        animation: Animation,
        bounds: Rectangle,
        content: Size,
    );

    /// Scroll the widget by the given [`AbsoluteOffset`] along the horizontal & vertical axis.
    fn scroll_by(
        &mut self,
        offset: AbsoluteOffset,
        animation: Animation,
        bounds: Rectangle,
        content: Size,
    );
}

/// Produces an [`Operation`] that snaps the widget with the given [`Id`] to
/// the provided `percentage`.
pub fn snap_to<T>(
    target: Id,
    offset: RelativeOffset<Option<f32>>,
    animation: Animation,
) -> impl Operation<T> {
    struct SnapTo {
        target: Id,
        offset: RelativeOffset<Option<f32>>,
        animation: Animation,
    }

    impl<T> Operation<T> for SnapTo {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
            operate(self);
        }

        fn scrollable(
            &mut self,
            id: Option<&Id>,
            bounds: Rectangle,
            content: Size,
            _translation: Vector,
            state: &mut dyn Scrollable,
        ) {
            if Some(&self.target) == id {
                state.snap_to(self.offset, self.animation, bounds, content);
            }
        }
    }

    SnapTo {
        target,
        offset,
        animation,
    }
}

/// Produces an [`Operation`] that scrolls the widget with the given [`Id`] to
/// the provided [`AbsoluteOffset`].
pub fn scroll_to<T>(
    target: Id,
    offset: AbsoluteOffset<Option<f32>>,
    animation: Animation,
) -> impl Operation<T> {
    struct ScrollTo {
        target: Id,
        offset: AbsoluteOffset<Option<f32>>,
        animation: Animation,
    }

    impl<T> Operation<T> for ScrollTo {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
            operate(self);
        }

        fn scrollable(
            &mut self,
            id: Option<&Id>,
            bounds: Rectangle,
            content: Size,
            _translation: Vector,
            state: &mut dyn Scrollable,
        ) {
            if Some(&self.target) == id {
                state.scroll_to(self.offset, self.animation, bounds, content);
            }
        }
    }

    ScrollTo {
        target,
        offset,
        animation,
    }
}

/// Produces an [`Operation`] that scrolls the widget with the given [`Id`] by
/// the provided [`AbsoluteOffset`].
pub fn scroll_by<T>(target: Id, offset: AbsoluteOffset, animation: Animation) -> impl Operation<T> {
    struct ScrollBy {
        target: Id,
        offset: AbsoluteOffset,
        animation: Animation,
    }

    impl<T> Operation<T> for ScrollBy {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
            operate(self);
        }

        fn scrollable(
            &mut self,
            id: Option<&Id>,
            bounds: Rectangle,
            content: Size,
            _translation: Vector,
            state: &mut dyn Scrollable,
        ) {
            if Some(&self.target) == id {
                state.scroll_by(self.offset, self.animation, bounds, content);
            }
        }
    }

    ScrollBy {
        target,
        offset,
        animation,
    }
}

/// Produces an [`Operation`] that scrolls the widget with the given [`Id`] by
/// the minimum amount needed to bring `target` into view.
///
/// The `target` is expressed in the coordinate space of the scrollable
/// content, i.e. relative to its top-left corner and independent of the
/// current scroll offset. Axes along which `target` is already fully visible
/// are left untouched. If `target` is larger than the viewport along an axis,
/// its start is aligned with the start of the viewport.
pub fn reveal<T>(target: Id, rectangle: Rectangle, animation: Animation) -> impl Operation<T> {
    Reveal {
        target,
        x: Some((rectangle.x, rectangle.x + rectangle.width)),
        y: Some((rectangle.y, rectangle.y + rectangle.height)),
        animation,
    }
}

/// Produces an [`Operation`] that scrolls the widget with the given [`Id`]
/// vertically by the minimum amount needed to bring the span of `height`
/// starting at `y` into view, leaving the horizontal scroll untouched.
///
/// Like in [`reveal`], `y` is relative to the top of the scrollable content.
pub fn reveal_vertical<T>(
    target: Id,
    y: f32,
    height: f32,
    animation: Animation,
) -> impl Operation<T> {
    Reveal {
        target,
        x: None,
        y: Some((y, y + height)),
        animation,
    }
}

struct Reveal {
    target: Id,
    x: Option<(f32, f32)>,
    y: Option<(f32, f32)>,
    animation: Animation,
}

impl<T> Operation<T> for Reveal {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
        operate(self);
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content: Size,
        translation: Vector,
        state: &mut dyn Scrollable,
    ) {
        if Some(&self.target) != id {
            return;
        }

        let x = self
            .x
            .and_then(|(start, end)| reveal_offset(translation.x, bounds.width, start, end));

        let y = self
            .y
            .and_then(|(start, end)| reveal_offset(translation.y, bounds.height, start, end));

        if x.is_some() || y.is_some() {
            state.scroll_to(AbsoluteOffset { x, y }, self.animation, bounds, content);
        }
    }
}

/// Returns the offset needed to bring the span `start..end` into a viewport
/// of the given `length` currently at `offset`, or `None` if it is already
/// fully visible.
fn reveal_offset(offset: f32, length: f32, start: f32, end: f32) -> Option<f32> {
    if start < offset || end - start > length {
        Some(start)
    } else if end > offset + length {
        Some(end - length)
    } else {
        None
    }
}

/// The amount of absolute offset in each direction of a [`Scrollable`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AbsoluteOffset<T = f32> {
    /// The amount of horizontal offset
    pub x: T,
    /// The amount of vertical offset
    pub y: T,
}

impl From<AbsoluteOffset> for AbsoluteOffset<Option<f32>> {
    fn from(offset: AbsoluteOffset) -> Self {
        Self {
            x: Some(offset.x),
            y: Some(offset.y),
        }
    }
}

impl std::ops::Add<Vector> for AbsoluteOffset {
    type Output = Self;

    fn add(self, translation: Vector) -> Self::Output {
        Self {
            x: self.x + translation.x,
            y: self.y + translation.y,
        }
    }
}

/// The amount of relative offset in each direction of a [`Scrollable`].
///
/// A value of `0.0` means start, while `1.0` means end.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RelativeOffset<T = f32> {
    /// The amount of horizontal offset
    pub x: T,
    /// The amount of vertical offset
    pub y: T,
}

impl RelativeOffset {
    /// A relative offset that points to the top-left of a [`Scrollable`].
    pub const START: Self = Self { x: 0.0, y: 0.0 };

    /// A relative offset that points to the bottom-right of a [`Scrollable`].
    pub const END: Self = Self { x: 1.0, y: 1.0 };
}

impl From<RelativeOffset> for RelativeOffset<Option<f32>> {
    fn from(offset: RelativeOffset) -> Self {
        Self {
            x: Some(offset.x),
            y: Some(offset.y),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_span_is_left_untouched() {
        assert_eq!(reveal_offset(100.0, 50.0, 110.0, 140.0), None);
        assert_eq!(reveal_offset(100.0, 50.0, 100.0, 150.0), None);
    }

    #[test]
    fn span_above_aligns_to_start() {
        assert_eq!(reveal_offset(100.0, 50.0, 60.0, 80.0), Some(60.0));
    }

    #[test]
    fn span_below_aligns_to_end() {
        assert_eq!(reveal_offset(100.0, 50.0, 160.0, 180.0), Some(130.0));
    }

    #[test]
    fn partially_visible_span_moves_minimally() {
        assert_eq!(reveal_offset(100.0, 50.0, 90.0, 110.0), Some(90.0));
        assert_eq!(reveal_offset(100.0, 50.0, 140.0, 160.0), Some(110.0));
    }

    #[test]
    fn oversized_span_aligns_to_start() {
        assert_eq!(reveal_offset(0.0, 50.0, 10.0, 90.0), Some(10.0));
    }

    #[derive(Default)]
    struct Probe {
        scrolled_to: Option<AbsoluteOffset<Option<f32>>>,
    }

    impl Scrollable for Probe {
        fn snap_to(&mut self, _: RelativeOffset<Option<f32>>, _: Animation, _: Rectangle, _: Size) {
        }

        fn scroll_to(
            &mut self,
            offset: AbsoluteOffset<Option<f32>>,
            _: Animation,
            _: Rectangle,
            _: Size,
        ) {
            self.scrolled_to = Some(offset);
        }

        fn scroll_by(&mut self, _: AbsoluteOffset, _: Animation, _: Rectangle, _: Size) {}
    }

    fn run(id: Option<&Id>, translation: Vector) -> Probe {
        let mut operation = reveal::<()>(
            Id::new("list"),
            Rectangle::new(crate::Point::new(0.0, 200.0), Size::new(10.0, 20.0)),
            Animation::Instant,
        );
        let mut probe = Probe::default();

        Operation::<()>::scrollable(
            &mut operation,
            id,
            Rectangle::new(crate::Point::ORIGIN, Size::new(10.0, 100.0)),
            Size::new(10.0, 1000.0),
            translation,
            &mut probe,
        );

        probe
    }

    #[test]
    fn reveal_vertical_never_touches_the_horizontal_axis() {
        let mut operation = reveal_vertical::<()>(Id::new("list"), 200.0, 20.0, Animation::Instant);
        let mut probe = Probe::default();

        Operation::<()>::scrollable(
            &mut operation,
            Some(&Id::new("list")),
            Rectangle::new(crate::Point::ORIGIN, Size::new(10.0, 100.0)),
            Size::new(500.0, 1000.0),
            Vector::new(300.0, 0.0),
            &mut probe,
        );

        assert_eq!(
            probe.scrolled_to,
            Some(AbsoluteOffset {
                x: None,
                y: Some(120.0)
            })
        );
    }

    #[test]
    fn reveal_scrolls_only_the_matching_widget_on_the_needed_axis() {
        let probe = run(Some(&Id::new("list")), Vector::ZERO);

        assert_eq!(
            probe.scrolled_to,
            Some(AbsoluteOffset {
                x: None,
                y: Some(120.0)
            })
        );

        assert!(
            run(Some(&Id::new("other")), Vector::ZERO)
                .scrolled_to
                .is_none()
        );
        assert!(run(None, Vector::ZERO).scrolled_to.is_none());
        assert!(
            run(Some(&Id::new("list")), Vector::new(0.0, 150.0))
                .scrolled_to
                .is_none()
        );
    }
}
