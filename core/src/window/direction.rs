use crate::mouse;
use crate::{Point, Size};

/// The cardinal directions relative to the center of a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Points to the top edge of a window.
    North,

    /// Points to the bottom edge of a window.
    South,

    /// Points to the right edge of a window.
    East,

    /// Points to the left edge of a window.
    West,

    /// Points to the top-right corner of a window.
    NorthEast,

    /// Points to the top-left corner of a window.
    NorthWest,

    /// Points to the bottom-right corner of a window.
    SouthEast,

    /// Points to the bottom-left corner of a window.
    SouthWest,
}

impl Direction {
    /// Returns the [`Direction`] of the window edge or corner found under the
    /// given `position`, or `None` if it lies in the interior (or outside)
    /// of the window.
    ///
    /// The `position` is relative to the top-left corner of the window,
    /// `size` is the window size and `border` is the thickness of the
    /// resize band along each edge, all in logical pixels. The corner
    /// regions are `border` pixels wide; use [`Direction::at_with_corner`]
    /// for larger corners.
    ///
    /// This is useful for windows without decorations, which need to
    /// implement their own resize handles together with
    /// [`window::drag_resize`](crate::window).
    ///
    /// ```
    /// use iced_core::window::Direction;
    /// use iced_core::{Point, Size};
    ///
    /// let size = Size::new(800.0, 600.0);
    ///
    /// assert_eq!(
    ///     Direction::at(Point::new(400.0, 2.0), size, 5.0),
    ///     Some(Direction::North)
    /// );
    /// assert_eq!(
    ///     Direction::at(Point::new(798.0, 598.0), size, 5.0),
    ///     Some(Direction::SouthEast)
    /// );
    /// assert_eq!(Direction::at(Point::new(400.0, 300.0), size, 5.0), None);
    /// ```
    pub fn at(position: Point, size: Size, border: f32) -> Option<Self> {
        Self::at_with_corner(position, size, border, border)
    }

    /// Like [`Direction::at`], but with a configurable `corner` size.
    ///
    /// Points within the border band that are closer than `corner` pixels to
    /// two adjacent edges resolve to a diagonal [`Direction`]. A `corner`
    /// smaller than `border` is treated as `border`.
    ///
    /// Returns `None` if the window size or `border` is not positive and
    /// finite, or if `position` is outside of the window.
    pub fn at_with_corner(
        position: Point,
        size: Size,
        border: f32,
        corner: f32,
    ) -> Option<Self> {
        let valid = |value: f32| value.is_finite() && value > 0.0;

        if !valid(size.width)
            || !valid(size.height)
            || !valid(border)
            || !position.x.is_finite()
            || !position.y.is_finite()
        {
            return None;
        }

        let Point { x, y } = position;

        if x < 0.0 || y < 0.0 || x >= size.width || y >= size.height {
            return None;
        }

        let corner = if corner.is_finite() {
            corner.max(border)
        } else {
            border
        };

        // Resolves which side a coordinate is close to; when the window is
        // so small that both bands overlap, the nearest side wins.
        let side = |value: f32, length: f32, band: f32| {
            let low = value < band;
            let high = value >= length - band;

            match (low, high) {
                (true, true) => Some(value >= length / 2.0),
                (true, false) => Some(false),
                (false, true) => Some(true),
                (false, false) => None,
            }
        };

        let in_border = side(x, size.width, border).is_some()
            || side(y, size.height, border).is_some();

        if !in_border {
            return None;
        }

        let horizontal = side(x, size.width, corner); // Some(true) = east
        let vertical = side(y, size.height, corner); // Some(true) = south

        match (horizontal, vertical) {
            (Some(false), Some(false)) => Some(Self::NorthWest),
            (Some(true), Some(false)) => Some(Self::NorthEast),
            (Some(false), Some(true)) => Some(Self::SouthWest),
            (Some(true), Some(true)) => Some(Self::SouthEast),
            _ => {
                // Not in a corner region: fall back to the plain edge bands.
                match (
                    side(x, size.width, border),
                    side(y, size.height, border),
                ) {
                    (Some(false), None) => Some(Self::West),
                    (Some(true), None) => Some(Self::East),
                    (None, Some(false)) => Some(Self::North),
                    (None, Some(true)) => Some(Self::South),
                    _ => None,
                }
            }
        }
    }

    /// Returns the resize [`mouse::Interaction`] that matches this
    /// [`Direction`].
    ///
    /// ```
    /// use iced_core::mouse;
    /// use iced_core::window::Direction;
    ///
    /// assert_eq!(
    ///     Direction::North.interaction(),
    ///     mouse::Interaction::ResizingVertically
    /// );
    /// ```
    pub fn interaction(self) -> mouse::Interaction {
        match self {
            Self::North | Self::South => mouse::Interaction::ResizingVertically,
            Self::East | Self::West => mouse::Interaction::ResizingHorizontally,
            Self::NorthEast | Self::SouthWest => {
                mouse::Interaction::ResizingDiagonallyUp
            }
            Self::NorthWest | Self::SouthEast => {
                mouse::Interaction::ResizingDiagonallyDown
            }
        }
    }
}

impl From<Direction> for mouse::Interaction {
    fn from(direction: Direction) -> Self {
        direction.interaction()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: Size = Size::new(100.0, 80.0);

    fn at(x: f32, y: f32) -> Option<Direction> {
        Direction::at(Point::new(x, y), SIZE, 4.0)
    }

    #[test]
    fn edges() {
        assert_eq!(at(50.0, 1.0), Some(Direction::North));
        assert_eq!(at(50.0, 79.0), Some(Direction::South));
        assert_eq!(at(1.0, 40.0), Some(Direction::West));
        assert_eq!(at(99.0, 40.0), Some(Direction::East));
    }

    #[test]
    fn corners() {
        assert_eq!(at(1.0, 1.0), Some(Direction::NorthWest));
        assert_eq!(at(99.0, 1.0), Some(Direction::NorthEast));
        assert_eq!(at(1.0, 79.0), Some(Direction::SouthWest));
        assert_eq!(at(99.0, 79.0), Some(Direction::SouthEast));
    }

    #[test]
    fn interior_and_outside() {
        assert_eq!(at(50.0, 40.0), None);
        assert_eq!(at(4.0, 4.0), None);
        assert_eq!(at(-1.0, 40.0), None);
        assert_eq!(at(100.0, 40.0), None);
        assert_eq!(at(50.0, 80.0), None);
    }

    #[test]
    fn larger_corner() {
        let at = |x, y| {
            Direction::at_with_corner(Point::new(x, y), SIZE, 4.0, 12.0)
        };

        assert_eq!(at(1.0, 10.0), Some(Direction::NorthWest));
        assert_eq!(at(10.0, 1.0), Some(Direction::NorthWest));
        assert_eq!(at(1.0, 20.0), Some(Direction::West));
        assert_eq!(at(20.0, 1.0), Some(Direction::North));
        assert_eq!(at(97.0, 77.0), Some(Direction::SouthEast));
        // Inside the corner square but outside the border band.
        assert_eq!(at(8.0, 8.0), None);
    }

    #[test]
    fn corner_smaller_than_border_is_clamped() {
        let result =
            Direction::at_with_corner(Point::new(1.0, 3.0), SIZE, 4.0, 0.0);

        assert_eq!(result, Some(Direction::NorthWest));
    }

    #[test]
    fn degenerate_sizes() {
        let p = Point::new(0.0, 0.0);

        assert_eq!(Direction::at(p, Size::new(0.0, 0.0), 4.0), None);
        assert_eq!(Direction::at(p, Size::new(-5.0, 10.0), 4.0), None);
        assert_eq!(Direction::at(p, Size::new(f32::NAN, 10.0), 4.0), None);
        assert_eq!(Direction::at(p, SIZE, 0.0), None);
        assert_eq!(Direction::at(p, SIZE, -1.0), None);
        assert_eq!(Direction::at(Point::new(f32::NAN, 1.0), SIZE, 4.0), None);
    }

    #[test]
    fn tiny_window_nearest_side_wins() {
        let size = Size::new(6.0, 100.0);

        assert_eq!(
            Direction::at(Point::new(1.0, 50.0), size, 4.0),
            Some(Direction::West)
        );
        assert_eq!(
            Direction::at(Point::new(5.0, 50.0), size, 4.0),
            Some(Direction::East)
        );
    }

    #[test]
    fn interactions() {
        use mouse::Interaction as I;

        assert_eq!(Direction::North.interaction(), I::ResizingVertically);
        assert_eq!(Direction::South.interaction(), I::ResizingVertically);
        assert_eq!(Direction::East.interaction(), I::ResizingHorizontally);
        assert_eq!(Direction::West.interaction(), I::ResizingHorizontally);
        assert_eq!(Direction::NorthEast.interaction(), I::ResizingDiagonallyUp);
        assert_eq!(Direction::SouthWest.interaction(), I::ResizingDiagonallyUp);
        assert_eq!(
            Direction::NorthWest.interaction(),
            I::ResizingDiagonallyDown
        );
        assert_eq!(
            Direction::SouthEast.interaction(),
            I::ResizingDiagonallyDown
        );
        assert_eq!(I::from(Direction::East), I::ResizingHorizontally);
    }
}
