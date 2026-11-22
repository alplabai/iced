#![allow(missing_docs)]
#![allow(dead_code)]

use iced_graphics as graphics;
use iced_graphics::core;

mod border_sides;
mod layer;
mod text;

#[cfg(feature = "image")]
mod raster;

#[cfg(feature = "svg")]
mod vector;

#[cfg(feature = "geometry")]
mod geometry;

use crate::core::backend;
use crate::core::border;
use crate::core::gradient;
use crate::core::image;
use crate::core::renderer;
use crate::core::{
    Background, Color, Font, Gradient, Point, Rectangle, Size, Transformation, Vector,
};
use crate::graphics::compositor;
use crate::graphics::mesh;
use crate::graphics::text::{Editor, Paragraph};
use crate::graphics::{Shell, Text, Viewport};

use std::collections::VecDeque;
use std::num::NonZeroU32;

const ACCURACY: f64 = 0.1;

pub struct Renderer {
    settings: renderer::Settings,
    layers: layer::Stack,
    text: text::Pipeline,
    #[cfg(feature = "image")]
    raster: raster::Pipeline,
    #[cfg(feature = "svg")]
    vector: vector::Pipeline,
    scale: Option<renderer::Scale>,
}

impl Renderer {
    pub fn new(settings: renderer::Settings) -> Self {
        Self {
            settings,
            layers: layer::Stack::new(),
            text: text::Pipeline::new(),
            #[cfg(feature = "image")]
            raster: raster::Pipeline::new(),
            #[cfg(feature = "svg")]
            vector: vector::Pipeline::new(),
            scale: None,
        }
    }

    pub fn layers(&mut self) -> &[layer::Layer] {
        self.layers.flush();
        self.layers.as_slice()
    }

    pub fn draw(
        &mut self,
        renderer: &mut vello_cpu::RenderContext,
        viewport: &Viewport,
        background_color: Color,
    ) {
        use vello_cpu::kurbo::Shape;

        let scale = vello_cpu::kurbo::Affine::scale(f64::from(viewport.scale_factor()));

        renderer.set_transform(scale);
        renderer.set_paint(into_color(background_color));
        renderer.fill_rect(&into_rect(Rectangle::with_size(viewport.logical_size())));

        self.layers.merge();

        let viewport_bounds = Rectangle::with_size(viewport.logical_size());

        for layer in self.layers.iter() {
            // The root layer is unbounded until `reset`; an infinite clip path
            // clips everything, so clip to the viewport instead.
            let Some(layer_bounds) = layer.bounds.intersection(&viewport_bounds) else {
                continue;
            };
            let physical_layer_bounds =
                layer_bounds * Transformation::scale(viewport.scale_factor());

            renderer.set_transform(scale);
            renderer.push_clip_path(&into_rect(layer_bounds).to_path(ACCURACY));

            for (quad, background) in &layer.quads {
                let shadow = quad.shadow;

                if shadow.color.a > 0.0 {
                    let bounds = into_rect(quad.bounds) + into_vec2(shadow.offset);

                    let border_radius = quad
                        .border
                        .radius
                        .top_left
                        .max(quad.border.radius.top_right)
                        .max(quad.border.radius.bottom_right)
                        .max(quad.border.radius.bottom_left);

                    renderer.set_paint(into_color(shadow.color));
                    renderer.fill_blurred_rounded_rect(&bounds, border_radius, shadow.blur_radius);
                }

                renderer.set_paint(into_background(background, quad.bounds));

                let is_square = quad.border.radius == border::Radius::default();

                if is_square {
                    renderer.fill_rect(&into_rect(quad.bounds));
                } else {
                    renderer.fill_path(&into_rounded_rect(quad.bounds, quad.border.radius));
                }

                match border_sides::resolve(&quad.border, quad.bounds) {
                    border_sides::Resolved::Uniform { width, color } => {
                        if width > 0.0 && color.a > 0.0 {
                            renderer.set_paint(into_color(color));
                            renderer.set_stroke(vello_cpu::kurbo::Stroke::new(f64::from(width)));

                            let bounds = quad.bounds.shrink(width / 2.0);

                            if is_square {
                                renderer.stroke_rect(&into_rect(bounds));
                            } else {
                                renderer
                                    .stroke_path(&into_rounded_rect(bounds, quad.border.radius));
                            }
                        }
                    }
                    border_sides::Resolved::Sides { widths, colors } => {
                        border_sides::draw_sides(
                            renderer,
                            quad.bounds,
                            quad.border.radius,
                            widths,
                            colors,
                        );
                    }
                }
            }

            renderer.reset_transform();

            #[cfg(feature = "geometry")]
            for group in &layer.primitives {
                use vello_cpu::kurbo;

                let Some(clip_bounds) = group.clip_bounds().intersection(&layer_bounds) else {
                    continue;
                };

                renderer.push_clip_path(
                    &into_rect(clip_bounds * viewport.scale_factor()).to_path(ACCURACY),
                );
                // Primitives are in the coordinates of their frame, not of
                // their clip, so a clipped child frame must not shift them.
                let transformation =
                    Transformation::scale(viewport.scale_factor()) * group.transformation();
                let translation = transformation.translation();

                renderer.set_transform(
                    kurbo::Affine::scale(f64::from(transformation.scale_factor())).then_translate(
                        kurbo::Vec2 {
                            x: f64::from(translation.x),
                            y: f64::from(translation.y),
                        },
                    ),
                );

                for primitive in group.as_slice() {
                    match primitive {
                        geometry::Primitive::Fill { path, paint, rule } => {
                            renderer.set_paint(paint.clone());
                            renderer.set_fill_rule(*rule);
                            renderer.fill_path(path);
                        }
                        geometry::Primitive::Stroke {
                            path,
                            paint,
                            stroke,
                        } => {
                            renderer.set_paint(paint.clone());
                            renderer.set_stroke(stroke.clone());
                            renderer.stroke_path(path);
                        }
                    }
                }

                renderer.reset_transform();
                renderer.pop_clip();
            }

            for image in &layer.images {
                match image {
                    #[cfg(feature = "image")]
                    iced_graphics::Image::Raster {
                        image,
                        bounds,
                        clip_bounds,
                    } => {
                        renderer.push_clip_path(&into_rounded_rect(
                            *clip_bounds * viewport.scale_factor(),
                            image.border_radius * viewport.scale_factor(),
                        ));

                        self.raster
                            .draw(image, *bounds, renderer, viewport.scale_factor());

                        renderer.pop_clip();
                    }
                    #[cfg(feature = "svg")]
                    iced_graphics::Image::Vector {
                        svg,
                        bounds,
                        clip_bounds,
                    } => {
                        renderer.push_clip_path(
                            &into_rect(*clip_bounds * viewport.scale_factor()).to_path(ACCURACY),
                        );

                        self.vector
                            .draw(svg, *bounds, renderer, viewport.scale_factor());

                        renderer.pop_clip();
                    }
                    #[cfg(not(feature = "image"))]
                    iced_graphics::Image::Raster { .. } => {}
                    #[cfg(not(feature = "svg"))]
                    iced_graphics::Image::Vector { .. } => {}
                }
            }

            for item in &layer.text {
                for text in item.as_slice() {
                    match text {
                        Text::Paragraph {
                            paragraph,
                            position,
                            color,
                            clip_bounds,
                            transformation,
                        } => {
                            let transformation =
                                Transformation::scale(viewport.scale_factor()) * *transformation;

                            renderer.push_clip_path(
                                &into_rect(*clip_bounds * transformation).to_path(ACCURACY),
                            );

                            self.text.draw_paragraph(
                                paragraph,
                                *position,
                                *color,
                                renderer,
                                transformation,
                            );

                            renderer.pop_clip();
                        }
                        Text::Editor {
                            editor,
                            position,
                            color,
                            clip_bounds,
                            transformation,
                        } => {
                            let transformation =
                                Transformation::scale(viewport.scale_factor()) * *transformation;

                            renderer.push_clip_path(
                                &into_rect(*clip_bounds * transformation).to_path(ACCURACY),
                            );

                            self.text.draw_editor(
                                editor,
                                *position,
                                *color,
                                renderer,
                                transformation,
                            );

                            renderer.pop_clip();
                        }
                        Text::Cached {
                            content,
                            bounds,
                            color,
                            size,
                            line_height,
                            font,
                            align_x,
                            align_y,
                            shaping,
                            wrapping,
                            ellipsis,
                            clip_bounds,
                        } => {
                            let transformation = Transformation::scale(viewport.scale_factor())
                                * item.transformation();

                            // `layer_bounds` is already in logical screen space:
                            // only the scale applies, not the item transformation.
                            let Some(clip_bounds) = (*clip_bounds * transformation)
                                .intersection(&physical_layer_bounds)
                            else {
                                continue;
                            };

                            renderer.push_clip_path(&into_rect(clip_bounds).to_path(ACCURACY));

                            self.text.draw_cached(
                                content,
                                *bounds,
                                *color,
                                *size,
                                *line_height,
                                *font,
                                *align_x,
                                *align_y,
                                *shaping,
                                *wrapping,
                                *ellipsis,
                                renderer,
                                transformation,
                            );

                            renderer.pop_clip();
                        }
                        Text::Raw {
                            raw,
                            transformation,
                        } => {
                            // Skip only this text: nothing is pushed yet, and the
                            // rest of the layer still draws.
                            let Some(buffer) = raw.buffer.upgrade() else {
                                continue;
                            };

                            let transformation =
                                Transformation::scale(viewport.scale_factor()) * *transformation;

                            let (width, height) = buffer.size();

                            let text_bounds = Rectangle::new(
                                raw.position,
                                Size::new(
                                    width.unwrap_or(layer_bounds.width),
                                    height.unwrap_or(layer_bounds.height),
                                ),
                            ) * transformation;

                            // Fork #3412/#3445: raw text respects its own clip bounds.
                            let Some(clip_bounds) = text_bounds
                                .intersection(&(raw.clip_bounds * transformation))
                                .and_then(|bounds| bounds.intersection(&physical_layer_bounds))
                            else {
                                continue;
                            };

                            renderer.push_clip_path(&into_rect(clip_bounds).to_path(ACCURACY));

                            self.text.draw_raw(
                                &buffer,
                                raw.position,
                                raw.color,
                                renderer,
                                transformation,
                            );

                            renderer.pop_clip();
                        }
                    }
                }
            }

            renderer.pop_clip();
        }

        self.text.trim_cache();

        #[cfg(feature = "image")]
        self.raster.trim_cache();

        #[cfg(feature = "svg")]
        self.vector.trim_cache();
    }
}

fn into_color(Color { r, g, b, a }: Color) -> vello_cpu::color::AlphaColor<vello_cpu::color::Srgb> {
    // `softbuffer` expects BGRA
    vello_cpu::color::AlphaColor::<vello_cpu::color::Srgb>::new([b, g, r, a])
}

fn into_background(background: &Background, bounds: Rectangle) -> vello_cpu::PaintType {
    match background {
        Background::Color(color) => vello_cpu::PaintType::Solid(into_color(*color)),
        Background::Gradient(gradient) => vello_cpu::PaintType::Gradient(match gradient {
            Gradient::Linear(gradient) => {
                let (start, end) = bounds.chord(gradient.angle);

                into_gradient(start, end, &gradient.stops)
            }
        }),
    }
}

fn into_gradient(
    start: Point,
    end: Point,
    stops: &[Option<gradient::ColorStop>],
) -> vello_cpu::peniko::Gradient {
    vello_cpu::peniko::Gradient {
        kind: vello_cpu::peniko::GradientKind::Linear(
            vello_cpu::peniko::LinearGradientPosition::new((start.x, start.y), (end.x, end.y)),
        ),
        stops: vello_cpu::peniko::ColorStops(
            stops
                .iter()
                .copied()
                .filter_map(|stop| {
                    let stop = stop?;

                    Some(vello_cpu::peniko::ColorStop {
                        offset: stop.offset,
                        color: vello_cpu::color::DynamicColor {
                            cs: vello_cpu::color::ColorSpaceTag::Srgb,
                            flags: vello_cpu::color::Flags::default(),
                            components: [stop.color.b, stop.color.g, stop.color.r, stop.color.a],
                        },
                    })
                })
                .collect(),
        ),
        ..vello_cpu::peniko::Gradient::default()
    }
}

fn into_rect(rectangle: Rectangle) -> vello_cpu::kurbo::Rect {
    vello_cpu::kurbo::Rect {
        x0: f64::from(rectangle.x),
        y0: f64::from(rectangle.y),
        x1: f64::from(rectangle.x + rectangle.width),
        y1: f64::from(rectangle.y + rectangle.height),
    }
}

fn into_point(point: Point) -> vello_cpu::kurbo::Point {
    vello_cpu::kurbo::Point::new(f64::from(point.x), f64::from(point.y))
}

fn into_vec2(vector: Vector) -> vello_cpu::kurbo::Vec2 {
    vello_cpu::kurbo::Vec2::new(f64::from(vector.x), f64::from(vector.y))
}

fn into_rounded_rect(rectangle: Rectangle, radius: border::Radius) -> vello_cpu::kurbo::BezPath {
    use vello_cpu::kurbo::Shape;

    into_rect(rectangle)
        .to_rounded_rect((
            f64::from(radius.top_left),
            f64::from(radius.top_right),
            f64::from(radius.bottom_right),
            f64::from(radius.bottom_left),
        ))
        .to_path(ACCURACY)
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
        _handle: &image::Handle,
        _callback: impl FnOnce(Result<image::Allocation, image::Error>) + Send + 'static,
    ) {
        // TODO: Concurrency
        #[cfg(feature = "image")]
        _callback(self.raster.load(_handle));
    }

    fn hint(&mut self, scale: renderer::Scale) {
        self.scale = Some(scale);
    }

    fn scale(&self) -> Option<renderer::Scale> {
        self.scale
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
        paragraph: &Self::Paragraph,
        position: core::Point,
        color: core::Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_paragraph(paragraph, position, color, clip_bounds, transformation);
    }

    fn fill_editor(
        &mut self,
        editor: &Self::Editor,
        position: core::Point,
        color: core::Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_editor(editor, position, color, clip_bounds, transformation);
    }

    fn fill_text(
        &mut self,
        text: core::Text<String>,
        position: core::Point,
        color: core::Color,
        clip_bounds: Rectangle,
    ) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text(text, position, color, clip_bounds, transformation);
    }
}

// Upstream's vello_cpu drops this impl, which tiny_skia had; the fallback
// renderer needs it from both backends to draw raw `cosmic_text` buffers.
impl graphics::text::Renderer for Renderer {
    fn fill_raw(&mut self, raw: graphics::text::Raw) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_text_raw(raw, transformation);
    }
}

#[cfg(feature = "geometry")]
impl graphics::geometry::Renderer for Renderer {
    type Geometry = geometry::Geometry;
    type Frame = geometry::Frame;

    fn new_frame(&self, bounds: Rectangle) -> Self::Frame {
        geometry::Frame::new(bounds)
    }

    fn draw_geometry(&mut self, geometry: Self::Geometry) {
        pub use geometry::Geometry;

        match geometry {
            Geometry::Live {
                sections,
                clip_bounds,
            } => {
                for (i, section) in sections.into_iter().enumerate() {
                    // A later section must draw above the earlier ones, which
                    // a single layer cannot guarantee.
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

                    layer.draw_text_group(section.text, clip_bounds, transformation);
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

                    layer.draw_text_cache(section.text.clone(), cache.clip_bounds, transformation);
                }
            }
        }
    }
}

#[cfg(feature = "image")]
impl image::Renderer for Renderer {
    type Handle = image::Handle;

    fn load_image(&self, handle: &image::Handle) -> Result<image::Allocation, image::Error> {
        self.raster.load(handle)
    }

    fn measure_image(&self, handle: &image::Handle) -> Option<core::Size<u32>> {
        self.raster.dimensions(handle)
    }

    fn draw_image(&mut self, image: core::Image, bounds: Rectangle, clip_bounds: Rectangle) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_raster(image, bounds, clip_bounds, transformation);
    }
}

#[cfg(feature = "svg")]
impl core::svg::Renderer for Renderer {
    fn measure_svg(&self, handle: &core::svg::Handle) -> core::Size<u32> {
        self.vector.viewport_dimensions(handle)
    }

    fn draw_svg(&mut self, svg: core::Svg, bounds: Rectangle, clip_bounds: Rectangle) {
        let (layer, transformation) = self.layers.current_mut();
        layer.draw_svg(svg, bounds, clip_bounds, transformation);
    }
}

impl mesh::Renderer for Renderer {
    fn draw_mesh(&mut self, _mesh: mesh::Mesh) {
        log::warn!("iced_vello_cpu does not support drawing meshes");
    }

    fn draw_mesh_cache(&mut self, _cache: mesh::Cache) {
        log::warn!("iced_vello_cpu does not support drawing meshes");
    }
}

impl compositor::Default for Renderer {
    type Compositor = Compositor;
}

pub struct Compositor {
    context: softbuffer::Context<Box<dyn compositor::Display>>,
    settings: backend::Settings,
}

#[derive(Debug, Clone)]
struct Frame {
    background: Color,
    scale_factor: f32,
    layers: Vec<layer::Layer>,
}

impl Frame {
    /// Whether layer damage against this frame is valid: a new background or
    /// scale factor repaints everything (fork port of upstream #3531).
    fn is_comparable(&self, background: Color, scale_factor: f32) -> bool {
        self.background == background && self.scale_factor == scale_factor
    }

    fn damage(&self, layers: &[layer::Layer]) -> Vec<Rectangle> {
        graphics::damage::diff(
            &self.layers,
            layers,
            |layer| vec![layer.bounds],
            layer::Layer::damage,
        )
    }
}

pub struct Surface {
    window: softbuffer::Surface<Box<dyn compositor::Display>, Box<dyn compositor::Window>>,
    renderer: vello_cpu::RenderContext,
    resources: vello_cpu::Resources,
    frames: VecDeque<Frame>,
    max_age: u8,
}

impl graphics::Compositor for Compositor {
    type Renderer = Renderer;
    type Surface = Surface;

    async fn new(
        settings: backend::Settings,
        display: impl compositor::Display + Clone,
        _compatible_window: impl compositor::Window + Clone,
        _shell: Shell,
    ) -> Result<Self, backend::Error> {
        if !settings.backend.is_software() && !settings.backend.matches("vello-cpu") {
            return Err(backend::Error::GraphicsAdapterNotFound {
                backend: "vello-cpu",
                reason: backend::Reason::DidNotMatch {
                    preferred_backend: settings.backend,
                },
            });
        }

        #[allow(unsafe_code)]
        let context =
            softbuffer::Context::new(Box::new(display) as _).expect("Create softbuffer context");

        Ok(Self { context, settings })
    }

    fn create_renderer(&self, settings: renderer::Settings) -> Renderer {
        Renderer::new(settings)
    }

    fn create_surface(&mut self, window: impl compositor::Window + Clone) -> Self::Surface {
        let window = softbuffer::Surface::new(&self.context, Box::new(window.clone()) as _)
            .expect("Create softbuffer surface for window");

        Surface {
            window,
            renderer: vello_cpu::RenderContext::new(1, 1),
            resources: vello_cpu::Resources::new(),
            frames: VecDeque::new(),
            max_age: 0,
        }
    }

    fn configure_surface(&mut self, surface: &mut Self::Surface, width: u32, height: u32) {
        surface
            .window
            .resize(
                NonZeroU32::new(width).expect("Non-zero width"),
                NonZeroU32::new(height).expect("Non-zero height"),
            )
            .expect("Resize surface");

        surface.renderer = vello_cpu::RenderContext::new(width as u16, height as u16);

        // The back buffers are reallocated, so all frame history is invalid.
        surface.frames.clear();
        surface.max_age = 0;
    }

    fn information(&self) -> compositor::Information {
        compositor::Information {
            adapter: String::from("CPU"),
            backend: String::from("vello-cpu"),
        }
    }

    fn present(
        &mut self,
        renderer: &mut Self::Renderer,
        surface: &mut Self::Surface,
        viewport: &iced_graphics::Viewport,
        background_color: core::Color,
        on_pre_present: impl FnOnce(),
    ) -> Result<(), compositor::SurfaceError> {
        let mut buffer = surface
            .window
            .buffer_mut()
            .map_err(|_| compositor::SurfaceError::Lost)?;

        let age = buffer.age();

        let old_frame = {
            surface.max_age = surface.max_age.max(age);
            surface.frames.truncate(surface.max_age as usize);

            if age > 0 {
                surface.frames.get(age as usize - 1)
            } else {
                None
            }
        };

        let damage = old_frame
            .and_then(|last_frame| {
                last_frame
                    .is_comparable(background_color, viewport.scale_factor())
                    .then(|| last_frame.damage(renderer.layers()))
            })
            .unwrap_or_else(|| vec![Rectangle::with_size(viewport.logical_size())]);

        let buffer_damage = if damage.is_empty() {
            vec![]
        } else {
            physical_damage(damage, viewport).collect()
        };

        let screen_damage = if age <= 1 {
            None
        } else {
            let damage = surface
                .frames
                .front()
                .and_then(|current_frame| {
                    current_frame
                        .is_comparable(background_color, viewport.scale_factor())
                        .then(|| current_frame.damage(renderer.layers()))
                })
                .unwrap_or_else(|| vec![Rectangle::with_size(viewport.logical_size())]);

            Some(physical_damage(damage, viewport).collect())
        };

        if screen_damage.as_ref().unwrap_or(&buffer_damage).is_empty() {
            return Ok(());
        }

        surface.frames.push_front(Frame {
            background: background_color,
            scale_factor: viewport.scale_factor(),
            layers: renderer.layers().to_vec(),
        });

        draw_to_buffer(
            renderer,
            &mut surface.renderer,
            &mut surface.resources,
            viewport,
            background_color,
            bytemuck::cast_slice_mut(&mut buffer),
            &buffer_damage,
        );

        on_pre_present();
        buffer
            .present_with_damage(&screen_damage.unwrap_or(buffer_damage))
            .map_err(|_| compositor::SurfaceError::Lost)
    }

    fn screenshot(
        &mut self,
        renderer: &mut Self::Renderer,
        viewport: &iced_graphics::Viewport,
        background_color: core::Color,
    ) -> Vec<u8> {
        screenshot(renderer, viewport, background_color)
    }
}

impl renderer::Headless for Renderer {
    async fn new(settings: renderer::Settings, backend: Option<&str>) -> Option<Self>
    where
        Self: Sized,
    {
        if backend.is_some_and(|backend| !["vello-cpu", "vello_cpu", "software"].contains(&backend))
        {
            return None;
        }

        Some(Self::new(settings))
    }

    fn name(&self) -> String {
        "vello_cpu".to_owned()
    }

    fn screenshot(
        &mut self,
        size: core::Size<u32>,
        scale_factor: f32,
        background_color: core::Color,
    ) -> Vec<u8> {
        screenshot(
            self,
            &Viewport::with_physical_size(
                size,
                renderer::Scale {
                    window: 1.0,
                    application: scale_factor,
                },
            ),
            background_color,
        )
    }
}

fn draw_to_buffer(
    renderer: &mut Renderer,
    vello: &mut vello_cpu::RenderContext,
    resources: &mut vello_cpu::Resources,
    viewport: &Viewport,
    background_color: Color,
    buffer: &mut [u8],
    damage: &[softbuffer::Rect],
) {
    use vello_cpu::kurbo::Shape;

    if damage.is_empty() {
        return;
    }

    vello.reset();

    // A translucent background would composite over the old buffer
    // contents (`SrcOver` target init), so clear the damaged regions
    // to transparent first.
    if background_color.a < 1.0 {
        let width = viewport.physical_width() as usize;

        for region in damage {
            for y in region.y..(region.y + u32::from(region.height)) {
                let pixel = (y as usize) * width + (region.x as usize);
                let start = pixel * 4;
                let end = start + (u32::from(region.width) as usize) * 4;
                buffer[start..end].fill(0);
            }
        }
    }

    let mut clip = vello_cpu::kurbo::BezPath::new();

    for region in damage {
        clip.extend(
            vello_cpu::kurbo::Rect {
                x0: f64::from(region.x),
                y0: f64::from(region.y),
                x1: f64::from(region.x + u32::from(region.width)),
                y1: f64::from(region.y + u32::from(region.height)),
            }
            .to_path(ACCURACY),
        );
    }

    vello.push_clip_path(&clip);

    let scale = vello_cpu::kurbo::Affine::scale(f64::from(viewport.scale_factor()));
    vello.set_transform(scale);

    renderer.draw(vello, viewport, background_color);

    vello.pop_clip();
    vello.flush();

    let pixmap = vello_cpu::PixmapMut::new(
        viewport.physical_width() as u16,
        viewport.physical_height() as u16,
        buffer,
    )
    .expect("Buffer size matches renderer dimensions");

    vello.render_with(
        pixmap,
        resources,
        vello_cpu::RasterizerSettings {
            target_init: vello_cpu::TargetInit::SrcOver,
            ..Default::default()
        },
    );
}

fn physical_damage(
    damage: Vec<Rectangle>,
    viewport: &Viewport,
) -> impl Iterator<Item = softbuffer::Rect> {
    let to_rect = |rectangle: Rectangle<u32>| {
        Some(softbuffer::Rect {
            x: rectangle.x,
            y: rectangle.y,
            width: NonZeroU32::new(rectangle.width)?,
            height: NonZeroU32::new(rectangle.height)?,
        })
    };

    graphics::damage::physical(
        graphics::damage::group(damage, Rectangle::with_size(viewport.logical_size())),
        viewport,
    )
    .filter_map(to_rect)
}

fn screenshot(renderer: &mut Renderer, viewport: &Viewport, background_color: Color) -> Vec<u8> {
    let mut vello = vello_cpu::RenderContext::new(
        viewport.physical_width() as u16,
        viewport.physical_height() as u16,
    );

    renderer.draw(&mut vello, viewport, background_color);
    vello.flush();

    let mut screenshot =
        vec![0; (viewport.physical_width() * viewport.physical_height()) as usize * 4];

    let pixmap = vello_cpu::PixmapMut::new(
        viewport.physical_width() as u16,
        viewport.physical_height() as u16,
        &mut screenshot,
    )
    .expect("Screenshot buffer size matches viewport dimensions");

    vello.render(pixmap, &mut vello_cpu::Resources::new());

    for i in 0..screenshot.len() / 4 {
        screenshot.swap(i * 4, i * 4 + 2);
    }

    screenshot
}

#[cfg(test)]
mod damage_clip {
    use super::*;
    use iced_graphics::core::Renderer as _;
    use iced_graphics::core::renderer::Quad;

    fn scene() -> Renderer {
        let mut renderer = Renderer::new(renderer::Settings::default());

        renderer.start_layer(Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        });

        // Red quad, top-left.
        renderer.fill_quad(
            Quad {
                bounds: Rectangle {
                    x: 10.0,
                    y: 10.0,
                    width: 30.0,
                    height: 30.0,
                },
                ..Default::default()
            },
            Background::Color(Color::from_rgb(1.0, 0.0, 0.0)),
        );

        // Green quad, bottom-right.
        renderer.fill_quad(
            Quad {
                bounds: Rectangle {
                    x: 60.0,
                    y: 60.0,
                    width: 30.0,
                    height: 30.0,
                },
                ..Default::default()
            },
            Background::Color(Color::from_rgb(0.0, 1.0, 0.0)),
        );

        renderer.end_layer();
        renderer
    }

    fn pixel(buffer: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * width + x) as usize) * 4;
        [buffer[i], buffer[i + 1], buffer[i + 2], buffer[i + 3]]
    }

    fn assert_incremental(scale: f32, background: Color) {
        let physical = (100.0 * scale) as u32;
        let viewport = Viewport::with_physical_size(
            Size::new(physical, physical),
            renderer::Scale {
                window: 1.0,
                application: scale,
            },
        );

        let mut vello = vello_cpu::RenderContext::new(physical as u16, physical as u16);

        // Full render (ground truth).
        let mut full = vec![0; (physical * physical) as usize * 4];
        draw_to_buffer(
            &mut scene(),
            &mut vello,
            &mut vello_cpu::Resources::new(),
            &viewport,
            background,
            &mut full,
            &full_damage(&viewport),
        );

        // Back buffer holding a distinct previous frame.
        let mut back = vec![0; (physical * physical) as usize * 4];
        for chunk in back.as_chunks_mut::<4>().0 {
            *chunk = [0, 0, 255, 255];
        }

        // Incrementally render the region around the red quad. The damage is
        // specified in logical coordinates and converted to the physical rects
        // that `draw_to_buffer` clips to (via the same helper `present()` uses).
        let damage = vec![Rectangle {
            x: 5.0,
            y: 5.0,
            width: 50.0,
            height: 50.0,
        }];
        let regions: Vec<_> = physical_damage(damage, &viewport).collect();

        draw_to_buffer(
            &mut scene(),
            &mut vello,
            &mut vello_cpu::Resources::new(),
            &viewport,
            background,
            &mut back,
            &regions,
        );

        // Sample points, in physical coordinates.
        let red_center = (30.0 * scale) as u32; // inside the red quad
        let background_point = (8.0 * scale) as u32; // inside the clip, plain background
        let green_center = (75.0 * scale) as u32; // inside the green quad

        // The damaged region matches a full render.
        assert_eq!(
            pixel(&back, physical, red_center, red_center),
            pixel(&full, physical, red_center, red_center)
        );
        assert_eq!(
            pixel(&back, physical, background_point, background_point),
            pixel(&full, physical, background_point, background_point)
        );

        // The undamaged region (green quad) keeps the previous content.
        assert_eq!(
            pixel(&back, physical, green_center, green_center),
            [0, 0, 255, 255]
        );
    }

    #[test]
    fn incremental_render_preserves_undamaged_pixels() {
        let background = Color::from_rgb(0.1, 0.1, 0.1);
        assert_incremental(1.0, background);
        assert_incremental(2.0, background);
    }

    #[test]
    fn incremental_render_with_translucent_background() {
        // The damaged regions are cleared to transparent before the
        // translucent background composites over them, so the old buffer
        // contents must not bleed through.
        let background = Color::from_rgba(0.1, 0.1, 0.1, 0.5);
        assert_incremental(1.0, background);
        assert_incremental(2.0, background);
    }

    #[test]
    fn translucent_background_full_render_clears_old_contents() {
        // A full-viewport render with a translucent background clears the
        // damaged regions to transparent first, so the old buffer contents
        // must not bleed through.
        let physical = 100u32;
        let viewport = Viewport::with_physical_size(
            Size::new(physical, physical),
            renderer::Scale {
                window: 1.0,
                application: 1.0,
            },
        );
        let background = Color::from_rgba(0.1, 0.1, 0.1, 0.5);

        let mut vello = vello_cpu::RenderContext::new(physical as u16, physical as u16);

        // Ground truth: a full render into a fresh (zeroed) buffer.
        let mut full = vec![0; (physical * physical) as usize * 4];
        draw_to_buffer(
            &mut scene(),
            &mut vello,
            &mut vello_cpu::Resources::new(),
            &viewport,
            background,
            &mut full,
            &full_damage(&viewport),
        );

        // Back buffer holding a distinct, opaque previous frame.
        let mut back = vec![0; (physical * physical) as usize * 4];
        for chunk in back.as_chunks_mut::<4>().0 {
            *chunk = [0, 0, 255, 255];
        }

        // Render the same scene in full over the stale back buffer.
        draw_to_buffer(
            &mut scene(),
            &mut vello,
            &mut vello_cpu::Resources::new(),
            &viewport,
            background,
            &mut back,
            &full_damage(&viewport),
        );

        // A plain background point must match the clean full render, i.e. the
        // stale opaque contents were cleared rather than blended through.
        let background_point = 8u32;
        assert_eq!(
            pixel(&back, physical, background_point, background_point),
            pixel(&full, physical, background_point, background_point),
        );
    }

    #[test]
    fn empty_damage_is_a_no_op() {
        // An empty damage list must leave the buffer untouched, as
        // `Compositor::present` relies on it to skip re-rendering a buffer
        // that already holds the current scene
        let physical = 100u32;
        let viewport = Viewport::with_physical_size(
            Size::new(physical, physical),
            renderer::Scale {
                window: 1.0,
                application: 1.0,
            },
        );

        let mut vello = vello_cpu::RenderContext::new(physical as u16, physical as u16);
        let mut buffer = vec![0; (physical * physical) as usize * 4];
        for chunk in buffer.as_chunks_mut::<4>().0 {
            *chunk = [9, 9, 9, 255];
        }

        draw_to_buffer(
            &mut scene(),
            &mut vello,
            &mut vello_cpu::Resources::new(),
            &viewport,
            Color::from_rgb(0.1, 0.1, 0.1),
            &mut buffer,
            &[],
        );

        // The buffer is untouched.
        assert_eq!(pixel(&buffer, physical, 0, 0), [9, 9, 9, 255]);
        assert_eq!(pixel(&buffer, physical, 50, 50), [9, 9, 9, 255]);
    }

    #[test]
    fn physical_damage_scales_clamps_and_drops() {
        // 100x100 physical at scale 2.0 => 50x50 logical.
        let viewport = Viewport::with_physical_size(
            Size::new(100, 100),
            renderer::Scale {
                window: 1.0,
                application: 2.0,
            },
        );

        fn as_tuple(rects: impl Iterator<Item = softbuffer::Rect>) -> Vec<(u32, u32, u32, u32)> {
            rects
                .map(|r| (r.x, r.y, r.width.get(), r.height.get()))
                .collect()
        }

        // Scaled then expanded by the 2px margin: (10,10,20,20)*2 = (20,20,40,40),
        // expanded to (18,18,44,44).
        let damage = vec![Rectangle {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
        }];
        assert_eq!(
            as_tuple(physical_damage(damage, &viewport)),
            vec![(18, 18, 44, 44)]
        );

        // Clamped at the top-left (the expand pushes it negative; the clipped
        // part is dropped, so the width shrinks accordingly).
        let damage = vec![Rectangle {
            x: 0.0,
            y: 0.0,
            width: 5.0,
            height: 5.0,
        }];
        assert_eq!(
            as_tuple(physical_damage(damage, &viewport)),
            vec![(0, 0, 12, 12)]
        );

        // Clamped at the bottom-right (extends past the viewport).
        let damage = vec![Rectangle {
            x: 45.0,
            y: 45.0,
            width: 20.0,
            height: 20.0,
        }];
        assert_eq!(
            as_tuple(physical_damage(damage, &viewport)),
            vec![(88, 88, 12, 12)]
        );

        // Entirely outside the viewport => dropped (zero area after clamping).
        let damage = vec![Rectangle {
            x: 55.0,
            y: 55.0,
            width: 10.0,
            height: 10.0,
        }];
        assert_eq!(physical_damage(damage, &viewport).count(), 0);
    }
}

#[cfg(test)]
mod shadow {
    use super::*;
    use iced_graphics::core::Renderer as _;
    use iced_graphics::core::renderer::Quad;

    fn shadowed_scene() -> Renderer {
        let mut renderer = Renderer::new(renderer::Settings::default());

        renderer.start_layer(Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        });

        renderer.fill_quad(
            Quad {
                bounds: Rectangle {
                    x: 10.0,
                    y: 10.0,
                    width: 20.0,
                    height: 20.0,
                },
                shadow: core::Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.8),
                    offset: core::Vector::new(5.0, 5.0),
                    blur_radius: 4.0,
                },
                ..Default::default()
            },
            Background::Color(Color::from_rgb(1.0, 1.0, 1.0)),
        );

        renderer.end_layer();
        renderer
    }

    fn pixel(buffer: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * width + x) as usize) * 4;
        [buffer[i], buffer[i + 1], buffer[i + 2], buffer[i + 3]]
    }

    fn assert_shadow(scale: f32) {
        let physical = (100.0 * scale) as u32;
        let viewport = Viewport::with_physical_size(
            Size::new(physical, physical),
            renderer::Scale {
                window: 1.0,
                application: scale,
            },
        );
        let background = Color::from_rgb(0.1, 0.1, 0.1);

        let mut vello = vello_cpu::RenderContext::new(physical as u16, physical as u16);
        let mut buffer = vec![0; (physical * physical) as usize * 4];

        draw_to_buffer(
            &mut shadowed_scene(),
            &mut vello,
            &mut vello_cpu::Resources::new(),
            &viewport,
            background,
            &mut buffer,
            &full_damage(&viewport),
        );

        let at = |x: f32, y: f32| pixel(&buffer, physical, (x * scale) as u32, (y * scale) as u32);

        // The quad is drawn on top of its shadow.
        assert!(at(20.0, 20.0)[0] > 200);

        // The shadow extends past the quad, offset towards the bottom-right.
        let background_pixel = at(3.0, 8.0);
        let shadow_pixel = at(37.0, 37.0);
        assert!(shadow_pixel[0] < background_pixel[0]);

        // Far away from the shadow, the background is untouched.
        assert_eq!(at(90.0, 90.0), background_pixel);
    }

    #[test]
    fn shadow_is_rendered() {
        assert_shadow(1.0);
        assert_shadow(2.0);
    }
}

#[cfg(test)]
fn full_damage(viewport: &Viewport) -> Vec<softbuffer::Rect> {
    physical_damage(
        vec![Rectangle::with_size(viewport.logical_size())],
        viewport,
    )
    .collect()
}

#[cfg(test)]
mod fork {
    use super::*;
    use iced_graphics::core::Renderer as _;
    use iced_graphics::core::renderer::Headless as _;
    use iced_graphics::core::renderer::Quad;

    fn red_quad(renderer: &mut Renderer) {
        renderer.fill_quad(
            Quad {
                bounds: Rectangle::with_size(Size::new(8.0, 8.0)),
                ..Quad::default()
            },
            Background::Color(Color::from_rgb(1.0, 0.0, 0.0)),
        );
    }

    #[test]
    fn a_headless_draw_without_reset_is_not_clipped_away() {
        let mut renderer = Renderer::new(renderer::Settings::default());
        red_quad(&mut renderer);

        let rgba = renderer.screenshot(Size::new(8, 8), 1.0, Color::BLACK);

        assert_eq!(rgba[..4], [255, 0, 0, 255]);
    }

    #[test]
    fn a_new_scale_factor_invalidates_frame_damage() {
        let frame = Frame {
            background: Color::BLACK,
            scale_factor: 1.0,
            layers: Vec::new(),
        };

        assert!(frame.is_comparable(Color::BLACK, 1.0));
        assert!(!frame.is_comparable(Color::BLACK, 2.0));
        assert!(!frame.is_comparable(Color::WHITE, 1.0));
    }

    /// Canvas text inside a layer away from the origin, drawn through a
    /// translation as a canvas in a scrollable or pane is.
    #[cfg(feature = "geometry")]
    #[test]
    fn canvas_text_in_a_translated_layer_is_drawn() {
        use crate::graphics::geometry::frame::Backend as _;
        use crate::graphics::geometry::{Renderer as _, Text};

        let offset = Vector::new(50.0, 50.0);
        let mut renderer = Renderer::new(renderer::Settings::default());

        renderer.start_layer(Rectangle::new(
            Point::new(offset.x, offset.y),
            Size::new(100.0, 100.0),
        ));
        renderer.start_transformation(Transformation::translate(offset.x, offset.y));

        let mut frame = renderer.new_frame(Rectangle::with_size(Size::new(100.0, 100.0)));
        frame.fill_text(Text {
            content: "HHHH".to_owned(),
            position: Point::new(10.0, 10.0),
            color: Color::WHITE,
            size: core::Pixels(20.0),
            ..Text::default()
        });
        renderer.draw_geometry(frame.into_geometry());

        renderer.end_transformation();
        renderer.end_layer();

        let rgba = renderer.screenshot(Size::new(200, 200), 1.0, Color::BLACK);
        let ink = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|[r, g, b, _]| (*r, *g, *b) != (0, 0, 0))
            .count();

        assert!(ink > 0, "canvas text in a translated layer drew no pixels");
    }
}

/// Fork #3412/#3445: raw text respects its own clip bounds and the layer's.
#[cfg(test)]
mod raw_text {
    use super::*;
    use crate::graphics::text::{self, Raw, Renderer as _, cosmic_text};
    use iced_graphics::core::Renderer as _;
    use iced_graphics::core::renderer::Headless as _;

    use std::borrow::Cow;
    use std::ops::Range;
    use std::sync::Arc;

    const WIDTH: u32 = 180;
    const HEIGHT: u32 = 60;
    const CLIP_EDGE: u32 = 48;

    #[test]
    fn respects_clip_bounds_and_keeps_inner_ink() {
        let pixels = draw_raw_text(
            (Some(WIDTH as f32), Some(HEIGHT as f32)),
            clip_edge_bounds(),
            canvas_bounds(),
        );

        assert!(has_ink(&pixels, 0..CLIP_EDGE));
        assert!(!has_ink(&pixels, CLIP_EDGE..WIDTH));
    }

    #[test]
    fn intersects_layer_clip_bounds() {
        let pixels = draw_raw_text(
            (Some(WIDTH as f32), Some(HEIGHT as f32)),
            canvas_bounds(),
            clip_edge_bounds(),
        );

        assert!(has_ink(&pixels, 0..CLIP_EDGE));
        assert!(!has_ink(&pixels, CLIP_EDGE..WIDTH));
    }

    #[test]
    fn without_a_complete_size_is_masked() {
        for size in [
            (None, None),
            (Some(WIDTH as f32), None),
            (None, Some(HEIGHT as f32)),
        ] {
            let pixels = draw_raw_text(size, clip_edge_bounds(), canvas_bounds());

            assert!(has_ink(&pixels, 0..CLIP_EDGE));
            assert!(!has_ink(&pixels, CLIP_EDGE..WIDTH));
        }
    }

    #[test]
    fn a_dropped_buffer_skips_only_its_text() {
        let mut renderer = Renderer::new(renderer::Settings::default());

        renderer.fill_raw(Raw {
            buffer: std::sync::Weak::new(),
            position: Point::ORIGIN,
            color: Color::WHITE,
            clip_bounds: canvas_bounds(),
        });
        renderer.fill_quad(
            renderer::Quad {
                bounds: canvas_bounds(),
                ..renderer::Quad::default()
            },
            Background::Color(Color::from_rgb(1.0, 0.0, 0.0)),
        );

        let rgba = renderer.screenshot(Size::new(WIDTH, HEIGHT), 1.0, Color::BLACK);

        assert_eq!(rgba[..4], [255, 0, 0, 255]);
    }

    /// Draws white `MMMMMMMM` at the origin on black and returns RGBA bytes.
    fn draw_raw_text(
        size: (Option<f32>, Option<f32>),
        raw_clip_bounds: Rectangle,
        layer_clip_bounds: Rectangle,
    ) -> Vec<u8> {
        let buffer = {
            let mut font_system = text::font_system().write().expect("Write font system");

            font_system.load_font(Cow::Borrowed(
                include_bytes!("../../graphics/fonts/FiraSans-Regular.ttf").as_slice(),
            ));

            let mut buffer =
                cosmic_text::Buffer::new(font_system.raw(), cosmic_text::Metrics::new(32.0, 40.0));

            buffer.set_size(size.0, size.1);
            buffer.set_wrap(cosmic_text::Wrap::None);
            buffer.set_text(
                "MMMMMMMM",
                &cosmic_text::Attrs::new().family(cosmic_text::Family::Name("Fira Sans")),
                cosmic_text::Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(font_system.raw(), false);

            Arc::new(buffer)
        };

        let mut renderer = Renderer::new(renderer::Settings::default());

        renderer.start_layer(layer_clip_bounds);
        renderer.fill_raw(Raw {
            buffer: Arc::downgrade(&buffer),
            position: Point::ORIGIN,
            color: Color::WHITE,
            clip_bounds: raw_clip_bounds,
        });
        renderer.end_layer();

        renderer.screenshot(Size::new(WIDTH, HEIGHT), 1.0, Color::BLACK)
    }

    fn canvas_bounds() -> Rectangle {
        Rectangle::new(Point::ORIGIN, Size::new(WIDTH as f32, HEIGHT as f32))
    }

    fn clip_edge_bounds() -> Rectangle {
        Rectangle::new(Point::ORIGIN, Size::new(CLIP_EDGE as f32, HEIGHT as f32))
    }

    fn has_ink(pixels: &[u8], x_range: Range<u32>) -> bool {
        (0..HEIGHT).any(|y| {
            x_range.clone().any(|x| {
                let i = ((y * WIDTH + x) * 4) as usize;
                pixels[i..i + 3] != [0, 0, 0]
            })
        })
    }
}
