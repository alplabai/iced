#![allow(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
pub mod window;

mod engine;
mod layer;
mod primitive;
mod text;

#[cfg(feature = "image")]
mod raster;

#[cfg(feature = "svg")]
mod vector;

#[cfg(feature = "geometry")]
pub mod geometry;

use iced_debug as debug;
pub use iced_graphics as graphics;
pub use iced_graphics::core;

pub use layer::Layer;
pub use primitive::Primitive;

#[cfg(feature = "geometry")]
pub use geometry::Geometry;

use crate::core::renderer;
use crate::core::{Background, Color, Font, Point, Rectangle, Size, Transformation};
use crate::engine::Engine;
use crate::graphics::Viewport;
use crate::graphics::compositor;
use crate::graphics::text::{Editor, Paragraph};

/// A [`tiny-skia`] graphics renderer for [`iced`].
///
/// [`tiny-skia`]: https://github.com/RazrFalcon/tiny-skia
/// [`iced`]: https://github.com/iced-rs/iced
#[derive(Debug)]
pub struct Renderer {
    settings: renderer::Settings,
    layers: layer::Stack,
    engine: Engine, // TODO: Shared engine
}

impl Renderer {
    pub fn new(settings: renderer::Settings) -> Self {
        Self {
            settings,
            layers: layer::Stack::new(),
            engine: Engine::new(),
        }
    }

    pub fn layers(&mut self) -> &[Layer] {
        self.layers.flush();
        self.layers.as_slice()
    }

    pub fn draw(
        &mut self,
        pixels: &mut tiny_skia::PixmapMut<'_>,
        clip_mask: &mut tiny_skia::Mask,
        viewport: &Viewport,
        damage: &[Rectangle],
        background_color: Color,
    ) {
        let scale_factor = viewport.scale_factor();
        self.layers.flush();

        // The mask may hold anything from the last frame: start from a full clear.
        clip_mask.clear();
        let mut mask_bounds = Rectangle::new(Point::ORIGIN, Size::ZERO);

        for &damage_bounds in damage {
            let damage_bounds = damage_bounds * scale_factor;

            let path = tiny_skia::PathBuilder::from_rect(
                tiny_skia::Rect::from_xywh(
                    damage_bounds.x,
                    damage_bounds.y,
                    damage_bounds.width,
                    damage_bounds.height,
                )
                .expect("Create damage rectangle"),
            );

            pixels.fill_path(
                &path,
                &tiny_skia::Paint {
                    shader: tiny_skia::Shader::SolidColor(engine::into_color(background_color)),
                    anti_alias: false,
                    blend_mode: tiny_skia::BlendMode::Source,
                    ..Default::default()
                },
                tiny_skia::FillRule::default(),
                tiny_skia::Transform::identity(),
                None,
            );

            for layer in self.layers.iter() {
                let Some(layer_bounds) = damage_bounds.intersection(&(layer.bounds * scale_factor))
                else {
                    continue;
                };

                engine::adjust_clip_mask(clip_mask, &mut mask_bounds, layer_bounds);

                if !layer.quads.is_empty() {
                    let render_span = debug::render(debug::Primitive::Quad);
                    for (quad, background) in &layer.quads {
                        self.engine.draw_quad(
                            quad,
                            background,
                            Transformation::scale(scale_factor),
                            pixels,
                            clip_mask,
                            layer_bounds,
                        );
                    }
                    render_span.finish();
                }

                if !layer.primitives.is_empty() {
                    let render_span = debug::render(debug::Primitive::Triangle);

                    for group in &layer.primitives {
                        let Some(group_bounds) =
                            (group.clip_bounds() * scale_factor).intersection(&layer_bounds)
                        else {
                            continue;
                        };

                        engine::adjust_clip_mask(clip_mask, &mut mask_bounds, group_bounds);

                        for primitive in group.as_slice() {
                            self.engine.draw_primitive(
                                primitive,
                                Transformation::scale(scale_factor) * group.transformation(),
                                pixels,
                                clip_mask,
                                group_bounds,
                            );
                        }

                        engine::adjust_clip_mask(clip_mask, &mut mask_bounds, layer_bounds);
                    }

                    render_span.finish();
                }

                if !layer.images.is_empty() {
                    let render_span = debug::render(debug::Primitive::Image);

                    for image in &layer.images {
                        self.engine.draw_image(
                            image,
                            Transformation::scale(scale_factor),
                            pixels,
                            clip_mask,
                            &mut mask_bounds,
                            layer_bounds,
                        );
                    }

                    render_span.finish();
                }

                if !layer.text.is_empty() {
                    let render_span = debug::render(debug::Primitive::Image);

                    for group in &layer.text {
                        let Some(group_bounds) =
                            (group.clip_bounds() * scale_factor).intersection(&layer_bounds)
                        else {
                            continue;
                        };

                        engine::adjust_clip_mask(clip_mask, &mut mask_bounds, group_bounds);

                        for text in group.as_slice() {
                            self.engine.draw_text(
                                text,
                                Transformation::scale(scale_factor) * group.transformation(),
                                pixels,
                                clip_mask,
                                &mut mask_bounds,
                                group_bounds,
                            );
                        }

                        engine::adjust_clip_mask(clip_mask, &mut mask_bounds, layer_bounds);
                    }

                    render_span.finish();
                }
            }
        }

        self.engine.trim();
    }
}

impl core::Renderer for Renderer {
    fn start_layer(&mut self, bounds: Rectangle) {
        self.layers.push_clip(bounds);
    }

    fn end_layer(&mut self) {
        self.layers.pop_clip();
    }

    fn start_transformation(&mut self, transformation: Transformation) {
        self.layers.push_transformation(transformation);
    }

    fn end_transformation(&mut self) {
        self.layers.pop_transformation();
    }

    fn fill_quad(&mut self, quad: renderer::Quad, background: impl Into<Background>) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_quad(quad, background.into(), transformation);
    }

    fn allocate_image(
        &self,
        _handle: &core::image::Handle,
        callback: impl FnOnce(Result<core::image::Allocation, core::image::Error>) + Send + 'static,
    ) {
        #[cfg(feature = "image")]
        #[allow(unsafe_code)]
        // TODO: Concurrency
        callback(self.engine.raster_pipeline.load(_handle));

        #[cfg(not(feature = "image"))]
        callback(Err(core::image::Error::Unsupported));
    }

    fn hint(&mut self, _scale: renderer::Scale) {
        // TODO: No hinting supported
        // We'll replace `tiny-skia` with `vello_cpu` soon
    }

    fn scale(&self) -> Option<renderer::Scale> {
        None
    }

    fn reset(&mut self, new_bounds: Rectangle) {
        self.layers.reset(new_bounds);
    }

    fn settings(&self) -> renderer::Settings {
        self.settings
    }
}

impl core::text::Renderer for Renderer {
    type Paragraph = Paragraph;
    type Editor = Editor;

    const ICON_FONT: Font = Font::new("Iced-Icons");
    const CHECKMARK_ICON: char = '\u{f00c}';
    const ARROW_DOWN_ICON: char = '\u{e800}';
    const ICED_LOGO: char = '\u{e801}';
    const SCROLL_UP_ICON: char = '\u{e802}';
    const SCROLL_DOWN_ICON: char = '\u{e803}';
    const SCROLL_LEFT_ICON: char = '\u{e804}';
    const SCROLL_RIGHT_ICON: char = '\u{e805}';

    fn fill_paragraph(
        &mut self,
        text: &Self::Paragraph,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();

        layer.draw_paragraph(text, position, color, clip_bounds, transformation);
    }

    fn fill_editor(
        &mut self,
        editor: &Self::Editor,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_editor(editor, position, color, clip_bounds, transformation);
    }

    fn fill_text(
        &mut self,
        text: core::Text,
        position: Point,
        color: Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text(text, position, color, clip_bounds, transformation);
    }
}

impl graphics::text::Renderer for Renderer {
    fn fill_raw(&mut self, raw: graphics::text::Raw) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text_raw(raw, transformation);
    }
}

#[cfg(feature = "geometry")]
impl graphics::geometry::Renderer for Renderer {
    type Geometry = Geometry;
    type Frame = geometry::Frame;

    fn new_frame(&self, bounds: Rectangle) -> Self::Frame {
        geometry::Frame::new(bounds)
    }

    fn draw_geometry(&mut self, geometry: Self::Geometry) {
        match geometry {
            Geometry::Live { sections } => {
                for (i, section) in sections.into_iter().enumerate() {
                    // Sections after the first one exist because their contents
                    // must be drawn above the previous ones, which a single
                    // layer cannot guarantee.
                    if i > 0 {
                        self.layers.split();
                    }

                    let (layer, transformation) = self.layers.current_mut();

                    for group in section.primitives {
                        layer.draw_primitive_group(group.items, group.clip_bounds, transformation);
                    }

                    for image in section.images {
                        layer.draw_image(image, transformation);
                    }

                    for group in section.text {
                        layer.draw_text_group(group.items, group.clip_bounds, transformation);
                    }
                }
            }
            Geometry::Cache(cache) => {
                for (i, section) in cache.sections.iter().enumerate() {
                    if i > 0 {
                        self.layers.split();
                    }

                    let (layer, transformation) = self.layers.current_mut();

                    for group in &section.primitives {
                        layer.draw_primitive_cache(
                            group.items.clone(),
                            group.clip_bounds,
                            transformation,
                        );
                    }

                    for image in section.images.iter() {
                        layer.draw_image(image.clone(), transformation);
                    }

                    for group in &section.text {
                        layer.draw_text_cache(
                            group.items.clone(),
                            group.clip_bounds,
                            transformation,
                        );
                    }
                }
            }
        }
    }
}

impl graphics::mesh::Renderer for Renderer {
    fn draw_mesh(&mut self, _mesh: graphics::Mesh) {
        log::warn!("iced_tiny_skia does not support drawing meshes");
    }

    fn draw_mesh_cache(&mut self, _cache: iced_graphics::mesh::Cache) {
        log::warn!("iced_tiny_skia does not support drawing meshes");
    }
}

#[cfg(feature = "image")]
impl core::image::Renderer for Renderer {
    type Handle = core::image::Handle;

    fn load_image(
        &self,
        handle: &Self::Handle,
    ) -> Result<core::image::Allocation, core::image::Error> {
        self.engine.raster_pipeline.load(handle)
    }

    fn measure_image(&self, handle: &Self::Handle) -> Option<crate::core::Size<u32>> {
        self.engine.raster_pipeline.dimensions(handle)
    }

    fn draw_image(&mut self, image: core::Image, bounds: Rectangle, clip_bounds: Rectangle) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_raster(image, bounds, clip_bounds, transformation);
    }
}

#[cfg(feature = "svg")]
impl core::svg::Renderer for Renderer {
    fn measure_svg(&self, handle: &core::svg::Handle) -> crate::core::Size<u32> {
        self.engine.vector_pipeline.viewport_dimensions(handle)
    }

    fn draw_svg(&mut self, svg: core::Svg, bounds: Rectangle, clip_bounds: Rectangle) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_svg(svg, bounds, clip_bounds, transformation);
    }
}

impl compositor::Default for Renderer {
    type Compositor = window::Compositor;
}

impl renderer::Headless for Renderer {
    async fn new(settings: renderer::Settings, backend: Option<&str>) -> Option<Self> {
        if backend.is_some_and(|backend| !["tiny-skia", "tiny_skia", "software"].contains(&backend))
        {
            return None;
        }

        Some(Self::new(settings))
    }

    fn name(&self) -> String {
        "tiny-skia".to_owned()
    }

    fn screenshot(
        &mut self,
        size: Size<u32>,
        scale_factor: f32,
        background_color: Color,
    ) -> Vec<u8> {
        let viewport = Viewport::with_physical_size(
            size,
            renderer::Scale {
                window: 1.0,
                application: scale_factor,
            },
        );

        window::compositor::screenshot(self, &viewport, background_color)
    }
}

#[cfg(all(test, feature = "geometry", feature = "image"))]
mod tests {
    use super::*;

    use crate::core::Renderer as _;
    use crate::core::image;
    use crate::core::renderer::Scale;
    use crate::graphics::cache::{self, Cached};
    use crate::graphics::geometry::Renderer as _;
    use crate::graphics::geometry::frame::Backend as _;

    const SIZE: u32 = 40;
    const BACKGROUND: Color = Color {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    };

    // Only colors that are symmetrical in their red and blue channels are used,
    // since `tiny-skia` stores pixels as BGRA.
    const GRAY: (u8, u8, u8) = (128, 128, 128);
    const GREEN: (u8, u8, u8) = (0, 255, 0);
    const WHITE: (u8, u8, u8) = (255, 255, 255);
    const BLACK: (u8, u8, u8) = (0, 0, 0);

    fn renderer() -> Renderer {
        let mut renderer = Renderer::new(renderer::Settings::default());
        renderer.reset(Rectangle::with_size(Size::new(SIZE as f32, SIZE as f32)));
        renderer
    }

    fn frame(renderer: &Renderer) -> geometry::Frame {
        renderer.new_frame(Rectangle::with_size(Size::new(SIZE as f32, SIZE as f32)))
    }

    fn rectangle(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(width, height))
    }

    fn white_image(size: u32) -> core::Image {
        core::Image::new(image::Handle::from_rgba(
            size,
            size,
            vec![255u8; (size * size * 4) as usize],
        ))
        .filter_method(image::FilterMethod::Nearest)
    }

    fn render(renderer: &mut Renderer) -> tiny_skia::Pixmap {
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

        pixmap
    }

    fn pixel(pixmap: &tiny_skia::Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let pixel = pixmap.pixel(x, y).expect("Pixel in bounds");

        (pixel.red(), pixel.green(), pixel.blue())
    }

    #[test]
    fn shapes_only_use_a_single_layer() {
        let mut renderer = renderer();
        let mut frame = frame(&renderer);

        frame.fill_rectangle(Point::new(0.0, 0.0), Size::new(20.0, 20.0), Color::BLACK);
        frame.fill_rectangle(Point::new(10.0, 10.0), Size::new(20.0, 20.0), Color::WHITE);

        renderer.draw_geometry(frame.into_geometry());

        assert_eq!(renderer.layers().len(), 1);

        let pixmap = render(&mut renderer);

        assert_eq!(pixel(&pixmap, 5, 5), BLACK);
        assert_eq!(pixel(&pixmap, 15, 15), WHITE);
        assert_eq!(pixel(&pixmap, 35, 35), GRAY);
    }

    #[test]
    fn shape_then_image_use_a_single_layer() {
        let mut renderer = renderer();
        let mut frame = frame(&renderer);

        frame.fill_rectangle(Point::new(0.0, 0.0), Size::new(40.0, 40.0), Color::BLACK);
        frame.draw_image(rectangle(10.0, 10.0, 20.0, 20.0), white_image(20));

        renderer.draw_geometry(frame.into_geometry());

        assert_eq!(renderer.layers().len(), 1);

        let pixmap = render(&mut renderer);

        assert_eq!(pixel(&pixmap, 5, 5), BLACK);
        assert_eq!(pixel(&pixmap, 15, 15), WHITE);
    }

    #[test]
    fn shapes_are_drawn_over_images_in_draw_order() {
        let mut renderer = renderer();
        let mut frame = frame(&renderer);

        // shape, image, shape
        frame.fill_rectangle(Point::new(0.0, 0.0), Size::new(40.0, 40.0), Color::BLACK);
        frame.draw_image(rectangle(10.0, 10.0, 20.0, 20.0), white_image(20));
        frame.fill_rectangle(
            Point::new(20.0, 20.0),
            Size::new(20.0, 20.0),
            Color::from_rgb(0.0, 1.0, 0.0),
        );

        renderer.draw_geometry(frame.into_geometry());

        assert_eq!(renderer.layers().len(), 2);

        let pixmap = render(&mut renderer);

        // Bottom shape
        assert_eq!(pixel(&pixmap, 5, 5), BLACK);
        // Image over the bottom shape
        assert_eq!(pixel(&pixmap, 15, 15), WHITE);
        // Top shape over both the image and the bottom shape
        assert_eq!(pixel(&pixmap, 25, 25), GREEN);
        assert_eq!(pixel(&pixmap, 35, 35), GREEN);
    }

    #[test]
    fn image_then_shape_is_drawn_in_draw_order() {
        let mut renderer = renderer();
        let mut frame = frame(&renderer);

        frame.draw_image(rectangle(0.0, 0.0, 20.0, 20.0), white_image(20));
        frame.fill_rectangle(Point::new(10.0, 10.0), Size::new(20.0, 20.0), Color::BLACK);

        renderer.draw_geometry(frame.into_geometry());

        let pixmap = render(&mut renderer);

        assert_eq!(pixel(&pixmap, 5, 5), WHITE);
        assert_eq!(pixel(&pixmap, 15, 15), BLACK);
        assert_eq!(pixel(&pixmap, 25, 25), BLACK);
        assert_eq!(pixel(&pixmap, 35, 35), GRAY);
    }

    #[test]
    fn cached_geometry_keeps_draw_order() {
        let mut renderer = renderer();
        let mut frame = frame(&renderer);

        frame.fill_rectangle(Point::new(0.0, 0.0), Size::new(40.0, 40.0), Color::BLACK);
        frame.draw_image(rectangle(10.0, 10.0, 20.0, 20.0), white_image(20));
        frame.fill_rectangle(
            Point::new(20.0, 20.0),
            Size::new(20.0, 20.0),
            Color::from_rgb(0.0, 1.0, 0.0),
        );

        let cache = frame.into_geometry().cache(cache::Group::unique(), None);

        renderer.draw_geometry(Geometry::load(&cache));

        assert_eq!(renderer.layers().len(), 2);

        let pixmap = render(&mut renderer);

        assert_eq!(pixel(&pixmap, 5, 5), BLACK);
        assert_eq!(pixel(&pixmap, 15, 15), WHITE);
        assert_eq!(pixel(&pixmap, 25, 25), GREEN);
    }

    #[test]
    fn clipped_frames_clip_shapes() {
        let mut renderer = renderer();
        let mut frame = frame(&renderer);

        let mut child = frame.draft(rectangle(10.0, 10.0, 20.0, 20.0));
        child.fill_rectangle(Point::new(0.0, 0.0), Size::new(40.0, 40.0), Color::BLACK);
        frame.paste(child);

        renderer.draw_geometry(frame.into_geometry());

        let pixmap = render(&mut renderer);

        assert_eq!(pixel(&pixmap, 5, 5), GRAY);
        assert_eq!(pixel(&pixmap, 15, 15), BLACK);
        assert_eq!(pixel(&pixmap, 35, 35), GRAY);
    }

    #[test]
    fn clipped_frames_clip_images() {
        let mut renderer = renderer();
        let mut frame = frame(&renderer);

        let mut child = frame.draft(rectangle(10.0, 10.0, 20.0, 20.0));
        child.draw_image(rectangle(0.0, 0.0, 40.0, 40.0), white_image(40));
        frame.paste(child);

        renderer.draw_geometry(frame.into_geometry());

        let pixmap = render(&mut renderer);

        assert_eq!(pixel(&pixmap, 5, 5), GRAY);
        assert_eq!(pixel(&pixmap, 15, 15), WHITE);
        assert_eq!(pixel(&pixmap, 35, 35), GRAY);
    }

    #[test]
    fn pasted_frames_keep_draw_order() {
        let mut renderer = renderer();
        let mut frame = frame(&renderer);

        frame.fill_rectangle(
            Point::new(0.0, 0.0),
            Size::new(40.0, 40.0),
            Color::from_rgb(0.0, 1.0, 0.0),
        );

        let mut child = frame.draft(rectangle(10.0, 10.0, 20.0, 20.0));
        child.fill_rectangle(Point::new(0.0, 0.0), Size::new(40.0, 40.0), Color::BLACK);
        frame.paste(child);

        frame.fill_rectangle(Point::new(15.0, 15.0), Size::new(10.0, 10.0), Color::WHITE);

        renderer.draw_geometry(frame.into_geometry());

        let pixmap = render(&mut renderer);

        assert_eq!(pixel(&pixmap, 5, 5), GREEN);
        assert_eq!(pixel(&pixmap, 12, 12), BLACK);
        assert_eq!(pixel(&pixmap, 20, 20), WHITE);
        assert_eq!(pixel(&pixmap, 35, 35), GREEN);
    }
}
