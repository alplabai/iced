//! Per-side border widths and colours (fork #3391).
use crate::core::border::{self, Border};
use crate::core::{Color, Rectangle};
use crate::{ACCURACY, into_color, into_rect, into_rounded_rect};

use vello_cpu::kurbo;
use vello_cpu::peniko::Fill;

/// A border resolved against its quad: either one stroke, or one band per side
/// in the order top, right, bottom, left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Resolved {
    Uniform {
        width: f32,
        color: Color,
    },
    Sides {
        widths: [f32; 4],
        colors: [Color; 4],
    },
}

pub fn resolve(border: &Border, bounds: Rectangle) -> Resolved {
    let sides = [border.top, border.right, border.bottom, border.left];

    // Keep the common case on the original uniform path.
    if sides
        .iter()
        .all(|side| side.color.is_none() && side.width.is_none())
    {
        return Resolved::Uniform {
            width: border.width,
            color: border.color,
        };
    }

    let colors = sides.map(|side| side.color.unwrap_or(border.color));
    let mut widths = sides.map(|side| side.width.unwrap_or(border.width).max(0.0));

    if widths.iter().all(|width| *width == widths[0]) {
        widths = [widths[0].min(bounds.width / 2.0).min(bounds.height / 2.0); 4];
    } else {
        // Opposite sides can never overlap.
        let [top, right, bottom, left] = &mut widths;

        shrink_to_fit(right, left, bounds.width);
        shrink_to_fit(top, bottom, bounds.height);
    }

    if colors.iter().all(|color| *color == colors[0]) && widths.iter().all(|w| *w == widths[0]) {
        Resolved::Uniform {
            width: widths[0],
            color: colors[0],
        }
    } else {
        Resolved::Sides { widths, colors }
    }
}

fn shrink_to_fit(a: &mut f32, b: &mut f32, available: f32) {
    let total = *a + *b;

    if total > available {
        let factor = available / total;

        *a *= factor;
        *b *= factor;
    }
}

pub fn draw_sides(
    renderer: &mut vello_cpu::RenderContext,
    bounds: Rectangle,
    radius: border::Radius,
    widths: [f32; 4],
    colors: [Color; 4],
) {
    let Some(first) = widths.iter().position(|width| *width > 0.0) else {
        return;
    };

    let outer = <[f32; 4]>::from(radius)
        .map(|radius| radius.min(bounds.width / 2.0).min(bounds.height / 2.0));

    if outer == [0.0; 4] {
        for side in (0..4).filter(|side| widths[*side] > 0.0) {
            renderer.set_paint(into_color(colors[side]));
            renderer.fill_path(&side_path(bounds, widths, side));
        }

        return;
    }

    let ring = ring_path(bounds, outer, widths);

    renderer.set_fill_rule(Fill::EvenOdd);

    if (0..4).all(|side| widths[side] == 0.0 || colors[side] == colors[first]) {
        renderer.set_paint(into_color(colors[first]));
        renderer.fill_path(&ring);
        renderer.set_fill_rule(Fill::NonZero);

        return;
    }

    renderer.push_clip_path(&ring);
    renderer.set_fill_rule(Fill::NonZero);

    for side in (0..4).filter(|side| widths[*side] > 0.0) {
        renderer.set_paint(into_color(colors[side]));
        renderer.fill_path(&side_path(bounds, widths, side));
    }

    renderer.pop_clip();
}

fn ring_path(bounds: Rectangle, outer: [f32; 4], widths: [f32; 4]) -> kurbo::BezPath {
    let [top, right, bottom, left] = widths;
    let [top_left, top_right, bottom_right, bottom_left] = outer;
    let mut path = into_rounded_rect(
        bounds,
        border::Radius {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        },
    );

    let inner = Rectangle {
        x: bounds.x + left,
        y: bounds.y + top,
        width: bounds.width - left - right,
        height: bounds.height - top - bottom,
    };

    if inner.width <= 0.0 || inner.height <= 0.0 {
        return path;
    }

    path.extend(elliptical_rect(
        inner,
        [
            ((top_left - left).max(0.0), (top_left - top).max(0.0)),
            ((top_right - right).max(0.0), (top_right - top).max(0.0)),
            (
                (bottom_right - right).max(0.0),
                (bottom_right - bottom).max(0.0),
            ),
            (
                (bottom_left - left).max(0.0),
                (bottom_left - bottom).max(0.0),
            ),
        ],
    ));

    path
}

fn side_path(bounds: Rectangle, widths: [f32; 4], side: usize) -> kurbo::BezPath {
    let [top, right, bottom, left] = widths;
    let (x, y) = (bounds.x, bounds.y);
    let (far_x, far_y) = (bounds.x + bounds.width, bounds.y + bounds.height);

    let points = match side {
        0 => [
            (x, y),
            (far_x, y),
            (far_x - right, y + top),
            (x + left, y + top),
        ],
        1 => [
            (far_x, y),
            (far_x, far_y),
            (far_x - right, far_y - bottom),
            (far_x - right, y + top),
        ],
        2 => [
            (far_x, far_y),
            (x, far_y),
            (x + left, far_y - bottom),
            (far_x - right, far_y - bottom),
        ],
        _ => [
            (x, far_y),
            (x, y),
            (x + left, y + top),
            (x + left, far_y - bottom),
        ],
    };

    let [first, rest @ ..] = points.map(|(x, y)| kurbo::Point::new(f64::from(x), f64::from(y)));
    let mut path = kurbo::BezPath::new();

    path.move_to(first);
    for point in rest {
        path.line_to(point);
    }
    path.close_path();

    path
}

/// A rounded rectangle whose corners are ellipses; kurbo only has circular ones.
fn elliptical_rect(bounds: Rectangle, radii: [(f32, f32); 4]) -> kurbo::BezPath {
    let [top_left, top_right, bottom_right, bottom_left] = radii.map(|(x, y)| {
        (
            f64::from(x.min(bounds.width / 2.0)),
            f64::from(y.min(bounds.height / 2.0)),
        )
    });

    let rect = into_rect(bounds);
    let mut path = kurbo::BezPath::new();

    path.move_to((rect.x0 + top_left.0, rect.y0));
    path.line_to((rect.x1 - top_right.0, rect.y0));
    arc_to(
        &mut path,
        (rect.x1 - top_right.0, rect.y0),
        (rect.x1, rect.y0 + top_right.1),
        top_right,
    );
    path.line_to((rect.x1, rect.y1 - bottom_right.1));
    arc_to(
        &mut path,
        (rect.x1, rect.y1 - bottom_right.1),
        (rect.x1 - bottom_right.0, rect.y1),
        bottom_right,
    );
    path.line_to((rect.x0 + bottom_left.0, rect.y1));
    arc_to(
        &mut path,
        (rect.x0 + bottom_left.0, rect.y1),
        (rect.x0, rect.y1 - bottom_left.1),
        bottom_left,
    );
    path.line_to((rect.x0, rect.y0 + top_left.1));
    arc_to(
        &mut path,
        (rect.x0, rect.y0 + top_left.1),
        (rect.x0 + top_left.0, rect.y0),
        top_left,
    );
    path.close_path();

    path
}

fn arc_to(
    path: &mut kurbo::BezPath,
    from: (f64, f64),
    to: (f64, f64),
    (radius_x, radius_y): (f64, f64),
) {
    if radius_x == 0.0 || radius_y == 0.0 {
        path.line_to(to);
        return;
    }

    let arc = kurbo::Arc::from_svg_arc(&kurbo::SvgArc {
        from: from.into(),
        to: to.into(),
        radii: kurbo::Vec2::new(radius_x, radius_y),
        x_rotation: 0.0,
        large_arc: false,
        sweep: true,
    });

    match arc {
        Some(arc) => arc.to_cubic_beziers(ACCURACY, |p1, p2, p| path.curve_to(p1, p2, p)),
        None => path.line_to(to),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Renderer;
    use crate::core::border::Side;
    use crate::core::renderer::{Headless as _, Quad};
    use crate::core::{Background, Renderer as _, Size, renderer};

    fn draw_border(border: Border) -> Vec<u8> {
        let mut renderer = Renderer::new(renderer::Settings::default());

        renderer.fill_quad(
            Quad {
                bounds: Rectangle {
                    x: 4.0,
                    y: 4.0,
                    width: 24.0,
                    height: 24.0,
                },
                border,
                ..Quad::default()
            },
            Background::Color(Color::WHITE),
        );

        renderer.screenshot(Size::new(32, 32), 1.0, Color::BLACK)
    }

    fn at(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
        let i = (y * 32 + x) * 4;

        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    }

    fn rgba(color: Color) -> [u8; 4] {
        [color.r, color.g, color.b, color.a].map(|channel| (channel * 255.0).round() as u8)
    }

    #[test]
    fn asymmetric_border_uses_each_side_and_allows_a_disabled_edge() {
        let pixels = draw_border(
            Border::default()
                .rounded(5)
                .width(3)
                .top(Side::default().color(Color::from_rgb(1.0, 0.0, 0.0)))
                .right(Side::default().color(Color::from_rgb(0.0, 1.0, 0.0)))
                .bottom(Side::default().width(0))
                .left(
                    Side::default()
                        .color(Color::from_rgb(0.0, 0.0, 1.0))
                        .width(5),
                ),
        );

        assert_ne!(at(&pixels, 16, 4), at(&pixels, 27, 16));
        assert_eq!(at(&pixels, 16, 27), at(&pixels, 16, 16));
    }

    #[test]
    fn resolved_uniform_border_uses_the_resolved_side_color() {
        let base = Color::from_rgb(1.0, 0.0, 0.0);
        let resolved = Color::from_rgb(0.0, 1.0, 0.0);
        let side = Side::default().color(resolved);
        let pixels = draw_border(
            Border::default()
                .color(base)
                .width(3)
                .top(side)
                .right(side)
                .bottom(side)
                .left(side),
        );

        assert_eq!(at(&pixels, 16, 5), rgba(resolved));
        assert_ne!(at(&pixels, 16, 5), rgba(base));
    }

    #[test]
    fn zero_radius_one_sided_border_only_paints_the_enabled_side() {
        let color = Color::from_rgb(1.0, 0.0, 0.0);
        let pixels = draw_border(
            Border::default()
                .color(color)
                .bottom(Side::default().width(3)),
        );

        assert_eq!(at(&pixels, 16, 26), rgba(color));
        assert_eq!(at(&pixels, 16, 5), rgba(Color::WHITE));
        assert_eq!(at(&pixels, 5, 16), rgba(Color::WHITE));
    }

    #[test]
    fn zero_radius_multicolor_border_skips_zero_width_sides() {
        let red = Color::from_rgb(1.0, 0.0, 0.0);
        let green = Color::from_rgb(0.0, 1.0, 0.0);
        let blue = Color::from_rgb(0.0, 0.0, 1.0);
        let pixels = draw_border(
            Border::default()
                .width(3)
                .top(Side::default().color(red))
                .right(Side::default().color(green))
                .bottom(Side::default().width(0))
                .left(Side::default().color(blue)),
        );

        assert_eq!(at(&pixels, 16, 5), rgba(red));
        assert_eq!(at(&pixels, 26, 16), rgba(green));
        assert_eq!(at(&pixels, 5, 16), rgba(blue));
        assert_eq!(at(&pixels, 16, 26), rgba(Color::WHITE));
    }

    #[test]
    fn rounded_same_color_border_preserves_asymmetric_widths() {
        let color = Color::from_rgb(0.0, 0.0, 1.0);
        let pixels = draw_border(
            Border::default()
                .color(color)
                .rounded(6)
                .width(2)
                .right(Side::default().width(5))
                .bottom(Side::default().width(0))
                .left(Side::default().width(4)),
        );

        assert_eq!(at(&pixels, 16, 5), rgba(color));
        assert_eq!(at(&pixels, 25, 16), rgba(color));
        assert_eq!(at(&pixels, 6, 16), rgba(color));
        assert_eq!(at(&pixels, 16, 26), rgba(Color::WHITE));
    }
}
