//! Fork #18: canvas geometry is drawn in call order and `Frame::with_clip`
//! child clips apply to what is drawn inside. Fork #22: canvas text far past
//! the old glyph atlas limit still draws, since vello_cpu has no atlas.
//!
//! Ported from the tiny_skia tests (`draw order` / `clipped frames`) when the
//! upstream sync replaced tiny_skia with vello_cpu.

use iced_graphics::cache::{self, Cached};
use iced_graphics::core::renderer::{self, Headless as _};
use iced_graphics::core::{
    self, Color, Pixels, Point, Rectangle, Renderer as _, Size, Transformation, image,
};
use iced_graphics::geometry::frame::Backend as _;
use iced_graphics::geometry::{MAX_ATLAS_TEXT_SIZE, Renderer as _, Text};
use iced_vello_cpu::Renderer;

const SIZE: u32 = 40;
const RED: [u8; 3] = [255, 0, 0];
const BLACK: [u8; 3] = [0, 0, 0];
const WHITE: [u8; 3] = [255, 255, 255];

type Geometry = <Renderer as iced_graphics::geometry::Renderer>::Geometry;
type Frame = <Renderer as iced_graphics::geometry::Renderer>::Frame;

fn renderer() -> Renderer {
    Renderer::new(renderer::Settings::default())
}

fn frame(renderer: &Renderer) -> Frame {
    renderer.new_frame(Rectangle::with_size(Size::new(SIZE as f32, SIZE as f32)))
}

fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(width, height))
}

fn text(size: f32) -> Text {
    Text {
        content: "H".to_owned(),
        position: Point::new(0.0, 0.0),
        color: Color::WHITE,
        size: Pixels(size),
        ..Text::default()
    }
}

fn cover(frame: &mut Frame) {
    frame.fill_rectangle(
        Point::ORIGIN,
        Size::new(SIZE as f32, SIZE as f32),
        Color::from_rgb(1.0, 0.0, 0.0),
    );
}

/// A black fill larger than the clip it is drawn in (10, 10, 20x20).
fn clipped_black(frame: &mut Frame) {
    let mut child = frame.draft(rectangle(10.0, 10.0, 20.0, 20.0));
    child.fill_rectangle(Point::ORIGIN, Size::new(40.0, 40.0), Color::BLACK);
    frame.paste(child);
}

fn render(renderer: &mut Renderer, background: Color) -> Vec<u8> {
    renderer.screenshot(Size::new(SIZE, SIZE), 1.0, background)
}

fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 3] {
    let start = ((y * SIZE + x) * 4) as usize;

    [rgba[start], rgba[start + 1], rgba[start + 2]]
}

fn all_pixels(rgba: &[u8], expected: [u8; 3]) -> bool {
    (0..SIZE).all(|x| (0..SIZE).all(|y| pixel(rgba, x, y) == expected))
}

fn has_ink(rgba: &[u8]) -> bool {
    (0..SIZE).any(|x| (0..SIZE).any(|y| pixel(rgba, x, y) != BLACK))
}

#[test]
fn a_shape_drawn_after_text_covers_the_text() {
    // Control: the text alone leaves ink on the black background.
    let mut with_text = renderer();
    let mut frame_with_text = frame(&with_text);
    frame_with_text.fill_text(text(30.0));
    with_text.draw_geometry(frame_with_text.into_geometry());
    assert!(has_ink(&render(&mut with_text, Color::BLACK)));

    let mut renderer = renderer();
    let mut frame = frame(&renderer);
    frame.fill_text(text(30.0));
    cover(&mut frame);
    renderer.draw_geometry(frame.into_geometry());

    assert_eq!(renderer.layers().len(), 2);
    assert!(all_pixels(&render(&mut renderer, Color::BLACK), RED));
}

#[test]
fn a_fill_inside_with_clip_does_not_paint_outside_the_clip() {
    let mut renderer = renderer();
    let mut frame = frame(&renderer);

    clipped_black(&mut frame);
    renderer.draw_geometry(frame.into_geometry());

    let rgba = render(&mut renderer, Color::WHITE);

    assert_eq!(pixel(&rgba, 5, 5), WHITE);
    assert_eq!(pixel(&rgba, 15, 15), BLACK);
    assert_eq!(pixel(&rgba, 35, 35), WHITE);
}

#[test]
fn pasted_frames_keep_draw_order() {
    let mut renderer = renderer();
    let mut frame = frame(&renderer);

    cover(&mut frame);
    clipped_black(&mut frame);
    frame.fill_rectangle(Point::new(15.0, 15.0), Size::new(10.0, 10.0), Color::WHITE);
    renderer.draw_geometry(frame.into_geometry());

    let rgba = render(&mut renderer, Color::WHITE);

    assert_eq!(pixel(&rgba, 5, 5), RED);
    assert_eq!(pixel(&rgba, 12, 12), BLACK);
    assert_eq!(pixel(&rgba, 20, 20), WHITE);
    assert_eq!(pixel(&rgba, 35, 35), RED);
}

#[test]
fn cached_geometry_keeps_draw_order() {
    let mut renderer = renderer();
    let mut frame = frame(&renderer);

    frame.fill_text(text(30.0));
    cover(&mut frame);

    let cache = frame.into_geometry().cache(cache::Group::unique(), None);
    renderer.draw_geometry(Geometry::load(&cache));

    assert_eq!(renderer.layers().len(), 2);
    assert!(all_pixels(&render(&mut renderer, Color::BLACK), RED));
}

#[test]
fn shapes_then_text_use_a_single_layer() {
    let mut renderer = renderer();
    let mut frame = frame(&renderer);

    frame.fill_rectangle(Point::ORIGIN, Size::new(10.0, 10.0), Color::BLACK);
    frame.fill_text(text(30.0));
    renderer.draw_geometry(frame.into_geometry());

    assert_eq!(renderer.layers().len(), 1);
}

/// vello_cpu has no glyph atlas, so canvas text far past the wgpu/tiny_skia
/// atlas limit (`MAX_ATLAS_TEXT_SIZE`) needs no outline fallback.
#[test]
fn oversize_canvas_text_still_renders_ink() {
    // A frame and viewport large enough to hold the whole glyph.
    let side = (3.0 * MAX_ATLAS_TEXT_SIZE) as u32;

    let mut renderer = renderer();
    let mut frame = renderer.new_frame(Rectangle::with_size(Size::new(side as f32, side as f32)));

    frame.fill_text(text(2.0 * MAX_ATLAS_TEXT_SIZE));
    renderer.draw_geometry(frame.into_geometry());

    let rgba = renderer.screenshot(Size::new(side, side), 1.0, Color::BLACK);

    assert!(
        rgba.as_chunks::<4>().0.iter().any(|p| p[..3] != BLACK),
        "oversize canvas text drew no pixels"
    );
}

/// The rows of the column `x` that hold `color`.
fn rows_with(rgba: &[u8], width: u32, x: u32, color: [u8; 3]) -> Vec<u32> {
    (0..rgba.len() as u32 / 4 / width)
        .filter(|y| {
            let start = ((y * width + x) * 4) as usize;

            rgba[start..start + 3] == color
        })
        .collect()
}

/// A canvas in a layer clipped to 40x40, drawn through `transformation`,
/// with a black fill at y 30..34 of its 40x60 frame.
fn black_fill_in_layer(transformation: Transformation, scale_factor: f32) -> Vec<u32> {
    let mut renderer = renderer();

    renderer.start_layer(rectangle(0.0, 0.0, 40.0, 40.0));
    renderer.with_transformation(transformation, |renderer| {
        let mut frame = renderer.new_frame(Rectangle::with_size(Size::new(40.0, 60.0)));
        frame.fill_rectangle(Point::new(0.0, 30.0), Size::new(40.0, 4.0), Color::BLACK);
        renderer.draw_geometry(frame.into_geometry());
    });
    renderer.end_layer();

    let side = (SIZE as f32 * scale_factor) as u32;
    let rgba = renderer.screenshot(Size::new(side, side), scale_factor, Color::WHITE);

    rows_with(&rgba, side, 5, BLACK)
}

#[test]
fn shapes_in_a_scrolled_clipped_canvas_are_drawn_in_place() {
    // Fill at y 30..34 scrolled up by 20: rows 10..14.
    assert_eq!(
        black_fill_in_layer(Transformation::translate(0.0, -20.0), 1.0),
        (10..14).collect::<Vec<_>>()
    );
}

#[test]
fn shapes_in_a_scrolled_clipped_canvas_are_drawn_in_place_at_scale_two() {
    // The same at 2x: rows 20..28.
    assert_eq!(
        black_fill_in_layer(Transformation::translate(0.0, -20.0), 2.0),
        (20..28).collect::<Vec<_>>()
    );
}

#[test]
fn shapes_follow_a_scaled_layer_transformation() {
    // Fill at y 30..34 at half size: rows 15..17.
    assert_eq!(
        black_fill_in_layer(Transformation::scale(0.5), 1.0),
        (15..17).collect::<Vec<_>>()
    );
}

const BLUE: [u8; 3] = [0, 0, 255];

fn white_image(size: u32) -> core::Image {
    core::Image::new(image::Handle::from_rgba(
        size,
        size,
        vec![255u8; (size * size * 4) as usize],
    ))
    .filter_method(image::FilterMethod::Nearest)
}

fn blue_pixel(renderer: &mut Renderer, x: u32, y: u32) -> [u8; 3] {
    pixel(&render(renderer, Color::from_rgb(0.0, 0.0, 1.0)), x, y)
}

#[test]
fn image_then_shape_is_drawn_in_draw_order() {
    let mut renderer = renderer();
    let mut frame = frame(&renderer);

    frame.draw_image(rectangle(0.0, 0.0, 20.0, 20.0), white_image(20));
    frame.fill_rectangle(Point::new(10.0, 10.0), Size::new(20.0, 20.0), Color::BLACK);
    renderer.draw_geometry(frame.into_geometry());

    let rgba = render(&mut renderer, Color::from_rgb(0.0, 0.0, 1.0));

    assert_eq!(pixel(&rgba, 5, 5), WHITE);
    assert_eq!(pixel(&rgba, 15, 15), BLACK);
    assert_eq!(pixel(&rgba, 25, 25), BLACK);
    assert_eq!(pixel(&rgba, 35, 35), BLUE);
}

#[test]
fn shapes_are_drawn_over_images_in_draw_order() {
    let mut renderer = renderer();
    let mut frame = frame(&renderer);

    frame.fill_rectangle(Point::ORIGIN, Size::new(40.0, 40.0), Color::BLACK);
    frame.draw_image(rectangle(10.0, 10.0, 20.0, 20.0), white_image(20));
    frame.fill_rectangle(
        Point::new(20.0, 20.0),
        Size::new(20.0, 20.0),
        Color::from_rgb(0.0, 1.0, 0.0),
    );
    renderer.draw_geometry(frame.into_geometry());

    assert_eq!(renderer.layers().len(), 2);

    let rgba = render(&mut renderer, Color::from_rgb(0.0, 0.0, 1.0));

    assert_eq!(pixel(&rgba, 5, 5), BLACK);
    assert_eq!(pixel(&rgba, 15, 15), WHITE);
    assert_eq!(pixel(&rgba, 25, 25), [0, 255, 0]);
    assert_eq!(pixel(&rgba, 35, 35), [0, 255, 0]);
}

#[test]
fn shape_then_image_use_a_single_layer() {
    let mut renderer = renderer();
    let mut frame = frame(&renderer);

    frame.fill_rectangle(Point::ORIGIN, Size::new(40.0, 40.0), Color::BLACK);
    frame.draw_image(rectangle(10.0, 10.0, 20.0, 20.0), white_image(20));
    renderer.draw_geometry(frame.into_geometry());

    assert_eq!(renderer.layers().len(), 1);
    assert_eq!(blue_pixel(&mut renderer, 5, 5), BLACK);
    assert_eq!(blue_pixel(&mut renderer, 15, 15), WHITE);
}

#[test]
fn shapes_only_use_a_single_layer() {
    let mut renderer = renderer();
    let mut frame = frame(&renderer);

    frame.fill_rectangle(Point::ORIGIN, Size::new(40.0, 40.0), Color::BLACK);
    frame.fill_rectangle(Point::new(10.0, 10.0), Size::new(20.0, 20.0), Color::WHITE);
    renderer.draw_geometry(frame.into_geometry());

    assert_eq!(renderer.layers().len(), 1);
}

#[test]
fn clipped_frames_clip_images() {
    let mut renderer = renderer();
    let mut frame = frame(&renderer);

    let mut child = frame.draft(rectangle(10.0, 10.0, 20.0, 20.0));
    child.draw_image(rectangle(0.0, 0.0, 40.0, 40.0), white_image(40));
    frame.paste(child);
    renderer.draw_geometry(frame.into_geometry());

    let rgba = render(&mut renderer, Color::from_rgb(0.0, 0.0, 1.0));

    assert_eq!(pixel(&rgba, 5, 5), BLUE);
    assert_eq!(pixel(&rgba, 15, 15), WHITE);
    assert_eq!(pixel(&rgba, 35, 35), BLUE);
}
