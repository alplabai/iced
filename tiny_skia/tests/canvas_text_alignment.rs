//! Pins the anchor meaning of `canvas::Text::position`: `Center` puts the
//! text's centre on the position, `Right`/`Bottom` put its right/bottom edge
//! there, for the atlas-style path (identity transform) and for the glyph mesh
//! path (rotated transform). Upstream iced-rs/iced#3192 reinterprets the
//! position as a box corner; these tests fail if that reaches the fork.

use iced_tiny_skia::Renderer;
use iced_tiny_skia::core::alignment::Vertical;
use iced_tiny_skia::core::renderer::{self, Scale};
use iced_tiny_skia::core::text::{Alignment, LineHeight};
use iced_tiny_skia::core::{Color, Pixels, Point, Radians, Rectangle, Renderer as _, Size, Vector};
use iced_tiny_skia::graphics::Viewport;
use iced_tiny_skia::graphics::geometry::frame::Backend as _;
use iced_tiny_skia::graphics::geometry::{Renderer as _, Text};

const SIZE: u32 = 400;
const BACKGROUND: Color = Color::BLACK;
const FONT_SIZE: f32 = 20.0;
const LINE: f32 = 30.0;
/// Side bearings and rounding of the ink against the advance box (measured up
/// to 4 px at a 20 px font; the vertical ink also sits inside the line box).
/// It cannot catch an offset smaller than about this much, so it guards the
/// anchor semantics (shifts of half a text width or a line), not subpixel layout.
const TOLERANCE: f32 = 5.0;

/// The bounding box of every pixel that is not background: `(x0, y0, x1, y1)`.
type Ink = (f32, f32, f32, f32);

fn text(content: &str, align_x: Alignment, align_y: Vertical, max_width: f32) -> Text {
    Text {
        content: content.to_owned(),
        position: Point::new(200.0, 200.0),
        max_width,
        color: Color::WHITE,
        size: Pixels(FONT_SIZE),
        line_height: LineHeight::Absolute(Pixels(LINE)),
        align_x,
        align_y,
        ..Text::default()
    }
}

/// Draws the text, through `transform` if given, and returns its ink.
fn ink(text: Text, transform: Option<(Vector, Radians)>) -> Ink {
    let mut renderer = Renderer::new(renderer::Settings::default());
    renderer.reset(Rectangle::with_size(Size::new(SIZE as f32, SIZE as f32)));

    let mut frame = renderer.new_frame(Rectangle::with_size(Size::new(SIZE as f32, SIZE as f32)));

    match transform {
        None => frame.fill_text(text),
        Some((translation, angle)) => {
            // The anchor becomes the origin of the rotated frame.
            frame.translate(translation);
            frame.rotate(angle);
            frame.fill_text(Text {
                position: Point::ORIGIN,
                ..text
            });
        }
    }

    renderer.draw_geometry(frame.into_geometry());

    let mut pixmap = tiny_skia::Pixmap::new(SIZE, SIZE).expect("Create pixmap");
    let mut mask = tiny_skia::Mask::new(SIZE, SIZE).expect("Create mask");
    let viewport = Viewport::with_physical_size(Size::new(SIZE, SIZE), Scale::default());

    renderer.draw(
        &mut pixmap.as_mut(),
        &mut mask,
        &viewport,
        &[Rectangle::with_size(Size::new(SIZE as f32, SIZE as f32))],
        BACKGROUND,
    );

    let mut ink = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);

    for y in 0..SIZE {
        for x in 0..SIZE {
            let pixel = pixmap.pixel(x, y).expect("Pixel in bounds");

            if (pixel.red(), pixel.green(), pixel.blue()) != (0, 0, 0) {
                ink = (
                    ink.0.min(x as f32),
                    ink.1.min(y as f32),
                    ink.2.max(x as f32 + 1.0),
                    ink.3.max(y as f32 + 1.0),
                );
            }
        }
    }

    assert!(ink.0 < ink.2, "Text drew no pixels");

    ink
}

fn close(actual: f32, expected: f32, what: &str) {
    assert!(
        (actual - expected).abs() <= TOLERANCE,
        "{what}: {actual} is not within {TOLERANCE} of {expected}"
    );
}

const ALIGN_X: [Alignment; 3] = [Alignment::Left, Alignment::Center, Alignment::Right];
const ALIGN_Y: [Vertical; 3] = [Vertical::Top, Vertical::Center, Vertical::Bottom];

/// Where the ink of a one-line text lands for every alignment pair, measured
/// against the `Left`/`Top` ink of the same text at the same position.
fn check_alignments(content: &str, lines: f32, max_width: f32, draw: impl Fn(Text) -> Ink) {
    let reference = draw(text(content, Alignment::Left, Vertical::Top, max_width));
    let width = reference.2 - reference.0;

    // Left/Top: the text starts at the anchor and hangs below it.
    close(reference.0, 200.0, "left edge of Left");
    assert!(reference.1 >= 200.0 - TOLERANCE && reference.3 <= 200.0 + lines * LINE + TOLERANCE);

    for align_x in ALIGN_X {
        for align_y in ALIGN_Y {
            let ink = draw(text(content, align_x, align_y, max_width));

            let shift_y = match align_y {
                Vertical::Top => 0.0,
                Vertical::Center => -lines * LINE / 2.0,
                Vertical::Bottom => -lines * LINE,
            };

            let case = format!("{align_x:?}/{align_y:?} max_width {max_width}");

            // The ink is narrower than the advance box by the side bearings,
            // so the expectations are the anchor itself, not a shift by the ink width.
            match align_x {
                Alignment::Center => close((ink.0 + ink.2) / 2.0, 200.0, &format!("{case} mid x")),
                Alignment::Right => close(ink.2, 200.0, &format!("{case} right edge")),
                _ => close(ink.0, 200.0, &format!("{case} left edge")),
            }

            assert!(ink.2 - ink.0 >= width - 1.0 && ink.2 - ink.0 <= width + 1.0);
            close(ink.1, reference.1 + shift_y, &format!("{case} y0"));
            close(ink.3, reference.3 + shift_y, &format!("{case} y1"));
        }
    }
}

#[test]
fn single_line_default_max_width_is_anchored() {
    check_alignments("HHHHHHHH", 1.0, f32::INFINITY, |text| ink(text, None));
}

#[test]
fn single_line_finite_max_width_is_anchored() {
    // The box is wider than the text: the anchor is still the position, not
    // the middle of the `max_width` box.
    check_alignments("HHHHHHHH", 1.0, 300.0, |text| ink(text, None));
}

#[test]
fn multi_line_text_is_anchored_as_a_block() {
    check_alignments("HHHHHHHH\nHHHH\nHH", 3.0, f32::INFINITY, |text| {
        ink(text, None)
    });
}

#[test]
fn wrapped_text_is_anchored_as_a_block() {
    // 8 glyphs per line at most: forces a wrap at a finite max_width.
    check_alignments("HHHH HHHH HHHH", 3.0, 70.0, |text| ink(text, None));
}

#[test]
fn all_alignments_are_anchored_through_a_translation() {
    // A scale/translation transform takes the other branch of `fill_text`.
    let anchor = Point::new(200.0, 200.0);
    let draw = |text: Text| {
        ink(
            Text {
                position: Point::ORIGIN,
                ..text
            },
            Some((Vector::new(anchor.x, anchor.y), Radians(0.0))),
        )
    };

    check_alignments("HHHHHHHH", 1.0, f32::INFINITY, draw);
}

#[test]
fn rotated_text_is_anchored_on_the_rotation_origin() {
    // 90 degrees clockwise: the text's x axis points down the canvas, its y
    // axis points left. Centre on the anchor means the ink's middle sits on
    // the anchor on both axes (within the half-line of bearings).
    let draw = |align_x, align_y| {
        ink(
            text("HHHHHHHH", align_x, align_y, f32::INFINITY),
            Some((
                Vector::new(200.0, 200.0),
                Radians(std::f32::consts::FRAC_PI_2),
            )),
        )
    };

    let top = draw(Alignment::Left, Vertical::Top);
    // Top: the line box starts on the anchor and hangs to its left.
    assert!(
        top.2 <= 200.0 + TOLERANCE,
        "rotated Top ink right of anchor"
    );

    for align_x in ALIGN_X {
        for align_y in ALIGN_Y {
            let ink = draw(align_x, align_y);
            let case = format!("rotated {align_x:?}/{align_y:?}");

            // Along the canvas y axis runs the text's x axis.
            match align_x {
                Alignment::Center => close((ink.1 + ink.3) / 2.0, 200.0, &format!("{case} mid y")),
                Alignment::Right => close(ink.3, 200.0, &format!("{case} end y")),
                _ => close(ink.1, 200.0, &format!("{case} start y")),
            }

            // Along the canvas x axis runs the text's y axis, reversed: Bottom
            // lies one line to the right of Top, Center half a line.
            let shift = match align_y {
                Vertical::Top => 0.0,
                Vertical::Center => LINE / 2.0,
                Vertical::Bottom => LINE,
            };

            close(ink.0, top.0 + shift, &format!("{case} x0"));
            close(ink.2, top.2 + shift, &format!("{case} x1"));
        }
    }
}
