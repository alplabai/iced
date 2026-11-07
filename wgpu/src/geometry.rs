//! Build and draw geometry.
use crate::core::text::LineHeight;
use crate::core::{self, Pixels, Point, Radians, Rectangle, Size, Svg, Transformation, Vector};
use crate::graphics::cache::{self, Cached};
use crate::graphics::color;
use crate::graphics::geometry::fill::{self, Fill};
use crate::graphics::geometry::section::{Kind, Sections};
use crate::graphics::geometry::{self, LineCap, LineDash, LineJoin, Path, Stroke, Style};
use crate::graphics::gradient::{self, Gradient};
use crate::graphics::mesh::{self, Mesh};
use crate::graphics::{Image, Text};
use crate::primitive;
use crate::text;

use lyon::geom::euclid;
use lyon::tessellation;

use std::borrow::Cow;
use std::mem;
use std::sync::Arc;

/// A part of a [`Geometry`] that can be drawn in a single layer, because
/// its contents are already in the order in which a layer draws them:
/// meshes first, then custom primitives, then images, and text last.
#[derive(Debug, Default)]
pub struct Section {
    pub meshes: Vec<Mesh>,
    pub primitives: Vec<primitive::Instance>,
    pub images: Vec<Image>,
    pub text: Vec<Text>,
}

/// The geometry of a [`Frame`].
///
/// Normally, a [`Geometry::Live`] contains a single [`Section`]. More than one
/// section is only needed when the draw order of the [`Frame`] could not be
/// preserved by a single layer (e.g. shapes drawn over an image).
#[derive(Debug)]
pub enum Geometry {
    Live { sections: Vec<Section> },
    Cached(Cache),
}

#[derive(Debug, Clone, Default)]
pub struct Cache {
    pub sections: Arc<[CachedSection]>,
}

#[derive(Debug, Clone, Default)]
pub struct CachedSection {
    pub meshes: Option<mesh::Cache>,
    pub primitives: Option<Arc<[primitive::Instance]>>,
    pub images: Option<Arc<[Image]>>,
    pub text: Option<text::Cache>,
}

impl Cached for Geometry {
    type Cache = Cache;

    fn load(cache: &Self::Cache) -> Self {
        Geometry::Cached(cache.clone())
    }

    fn cache(self, group: cache::Group, previous: Option<Self::Cache>) -> Self::Cache {
        match self {
            Self::Live { sections } => {
                let previous = previous.map(|previous| previous.sections);

                let sections: Vec<_> = sections
                    .into_iter()
                    .enumerate()
                    .map(|(i, section)| {
                        let previous = previous.as_ref().and_then(|sections| sections.get(i));

                        cache_section(section, group, previous.cloned())
                    })
                    .collect();

                Cache {
                    sections: Arc::from(sections),
                }
            }
            Self::Cached(cache) => cache,
        }
    }
}

fn cache_section(
    section: Section,
    group: cache::Group,
    previous: Option<CachedSection>,
) -> CachedSection {
    let Section {
        meshes,
        primitives,
        images,
        text,
    } = section;

    let primitives = if primitives.is_empty() {
        None
    } else {
        Some(Arc::from(primitives))
    };

    let images = if images.is_empty() {
        None
    } else {
        Some(Arc::from(images))
    };

    let meshes = Arc::from(meshes);

    if let Some(mut previous) = previous {
        if let Some(cache) = &mut previous.meshes {
            cache.update(meshes);
        } else {
            previous.meshes = if meshes.is_empty() {
                None
            } else {
                Some(mesh::Cache::new(meshes))
            };
        }

        if let Some(cache) = &mut previous.text {
            cache.update(text);
        } else {
            previous.text = text::Cache::new(group, text);
        }

        previous.primitives = primitives;
        previous.images = images;

        previous
    } else {
        CachedSection {
            meshes: if meshes.is_empty() {
                None
            } else {
                Some(mesh::Cache::new(meshes))
            },
            primitives,
            images,
            text: text::Cache::new(group, text),
        }
    }
}

/// A frame for drawing some geometry.
pub struct Frame {
    clip_bounds: Rectangle,
    buffers: BufferStack,
    sections: Sections<Section>,
    transforms: Transforms,
    fill_tessellator: tessellation::FillTessellator,
    stroke_tessellator: tessellation::StrokeTessellator,
}

impl Frame {
    /// Creates a new [`Frame`] with the given clip bounds.
    pub fn new(bounds: Rectangle) -> Frame {
        Frame {
            clip_bounds: bounds,
            buffers: BufferStack::new(),
            sections: Sections::new(),
            transforms: Transforms {
                previous: Vec::new(),
                current: Transform(lyon::math::Transform::identity()),
            },
            fill_tessellator: tessellation::FillTessellator::new(),
            stroke_tessellator: tessellation::StrokeTessellator::new(),
        }
    }

    /// Announces that something of the given [`Kind`] is about to be drawn.
    ///
    /// This makes sure that the draw order is preserved: if the current
    /// [`Section`] cannot hold the new content after what has been drawn
    /// already, a new one is started.
    fn begin(&mut self, kind: Kind) {
        let clip_bounds = self.clip_bounds;
        let buffers = &mut self.buffers;

        let _ = self.sections.begin_with(kind, |section| {
            section
                .meshes
                .extend(mem::replace(buffers, BufferStack::new()).into_meshes(clip_bounds));
        });
    }

    /// Turns any pending shape of the current [`Section`] into meshes.
    fn seal(&mut self) {
        let clip_bounds = self.clip_bounds;

        if self.buffers.is_empty() {
            return;
        }

        let meshes = mem::replace(&mut self.buffers, BufferStack::new()).into_meshes(clip_bounds);

        self.sections.current_mut().meshes.extend(meshes);
    }

    /// Finishes the [`Frame`] returning its sections in draw order.
    fn into_sections(mut self) -> Vec<Section> {
        let clip_bounds = self.clip_bounds;
        let buffers = &mut self.buffers;

        self.sections.finish(|section| {
            section
                .meshes
                .extend(mem::replace(buffers, BufferStack::new()).into_meshes(clip_bounds));
        })
    }
}

impl geometry::frame::Backend for Frame {
    type Geometry = Geometry;

    #[inline]
    fn width(&self) -> f32 {
        self.clip_bounds.width
    }

    #[inline]
    fn height(&self) -> f32 {
        self.clip_bounds.height
    }

    #[inline]
    fn size(&self) -> Size {
        self.clip_bounds.size()
    }

    #[inline]
    fn center(&self) -> Point {
        Point::new(self.clip_bounds.width / 2.0, self.clip_bounds.height / 2.0)
    }

    fn fill(&mut self, path: &Path, fill: impl Into<Fill>) {
        self.begin(Kind::Shape);

        let Fill { style, rule } = fill.into();

        let mut buffer = self
            .buffers
            .get_fill(&self.transforms.current.transform_style(style));

        let options = tessellation::FillOptions::default().with_fill_rule(into_fill_rule(rule));

        if self.transforms.current.is_identity() {
            self.fill_tessellator
                .tessellate_path(path.raw(), &options, buffer.as_mut())
        } else {
            let path = path.transform(&self.transforms.current.0);

            self.fill_tessellator
                .tessellate_path(path.raw(), &options, buffer.as_mut())
        }
        .expect("Tessellate path.");
    }

    fn fill_rectangle(&mut self, top_left: Point, size: Size, fill: impl Into<Fill>) {
        self.begin(Kind::Shape);

        let Fill { style, rule } = fill.into();

        let mut buffer = self
            .buffers
            .get_fill(&self.transforms.current.transform_style(style));

        let top_left = self
            .transforms
            .current
            .0
            .transform_point(lyon::math::Point::new(top_left.x, top_left.y));

        let size = self
            .transforms
            .current
            .0
            .transform_vector(lyon::math::Vector::new(size.width, size.height));

        let options = tessellation::FillOptions::default().with_fill_rule(into_fill_rule(rule));

        self.fill_tessellator
            .tessellate_rectangle(
                &lyon::math::Box2D::new(top_left, top_left + size),
                &options,
                buffer.as_mut(),
            )
            .expect("Fill rectangle");
    }

    fn stroke<'a>(&mut self, path: &Path, stroke: impl Into<Stroke<'a>>) {
        self.begin(Kind::Shape);

        let stroke = stroke.into();

        let mut buffer = self
            .buffers
            .get_stroke(&self.transforms.current.transform_style(stroke.style));

        let mut options = tessellation::StrokeOptions::default();
        options.line_width = stroke.width;
        options.start_cap = into_line_cap(stroke.line_cap);
        options.end_cap = into_line_cap(stroke.line_cap);
        options.line_join = into_line_join(stroke.line_join);

        let path = if stroke.line_dash.segments.is_empty() {
            Cow::Borrowed(path)
        } else {
            Cow::Owned(dashed(path, stroke.line_dash))
        };

        if self.transforms.current.is_identity() {
            self.stroke_tessellator
                .tessellate_path(path.raw(), &options, buffer.as_mut())
        } else {
            let path = path.transform(&self.transforms.current.0);

            self.stroke_tessellator
                .tessellate_path(path.raw(), &options, buffer.as_mut())
        }
        .expect("Stroke path");
    }

    fn stroke_rectangle<'a>(&mut self, top_left: Point, size: Size, stroke: impl Into<Stroke<'a>>) {
        self.begin(Kind::Shape);

        let stroke = stroke.into();

        let mut buffer = self
            .buffers
            .get_stroke(&self.transforms.current.transform_style(stroke.style));

        let top_left = self
            .transforms
            .current
            .0
            .transform_point(lyon::math::Point::new(top_left.x, top_left.y));

        let size = self
            .transforms
            .current
            .0
            .transform_vector(lyon::math::Vector::new(size.width, size.height));

        let mut options = tessellation::StrokeOptions::default();
        options.line_width = stroke.width;
        options.start_cap = into_line_cap(stroke.line_cap);
        options.end_cap = into_line_cap(stroke.line_cap);
        options.line_join = into_line_join(stroke.line_join);

        self.stroke_tessellator
            .tessellate_rectangle(
                &lyon::math::Box2D::new(top_left, top_left + size),
                &options,
                buffer.as_mut(),
            )
            .expect("Stroke rectangle");
    }

    fn stroke_text<'a>(&mut self, text: impl Into<geometry::Text>, stroke: impl Into<Stroke<'a>>) {
        let text = text.into();
        let stroke = stroke.into();

        text.draw_with(|glyph, _color| self.stroke(&glyph, stroke));
    }

    fn fill_text(&mut self, text: impl Into<geometry::Text>) {
        let text = text.into();

        let (scale_x, scale_y) = self.transforms.current.scale();

        if self.transforms.current.is_scale_translation()
            && scale_x == scale_y
            && scale_x > 0.0
            && scale_y > 0.0
            && text.fits_atlas(scale_y)
        {
            let (bounds, size, line_height) = if self.transforms.current.is_identity() {
                (
                    Rectangle::new(text.position, Size::new(text.max_width, f32::INFINITY)),
                    text.size,
                    text.line_height,
                )
            } else {
                let position = self.transforms.current.transform_point(text.position);

                let size = Pixels(text.size.0 * scale_y);

                let line_height = match text.line_height {
                    LineHeight::Absolute(size) => LineHeight::Absolute(Pixels(size.0 * scale_y)),
                    LineHeight::Relative(factor) => LineHeight::Relative(factor),
                };

                (
                    Rectangle::new(position, Size::new(text.max_width, f32::INFINITY)),
                    size,
                    line_height,
                )
            };

            self.begin(Kind::Text);

            self.sections.current_mut().text.push(Text::Cached {
                content: text.content,
                bounds,
                color: text.color,
                size,
                line_height: line_height.to_absolute(size),
                font: text.font,
                align_x: text.align_x,
                align_y: text.align_y,
                shaping: text.shaping,
                wrapping: text.wrapping,
                ellipsis: text.ellipsis,
                clip_bounds: self.clip_bounds,
            });
        } else {
            text.draw_with(|path, color| self.fill(&path, color));
        }
    }

    #[inline]
    fn translate(&mut self, translation: Vector) {
        self.transforms.current.0 = self
            .transforms
            .current
            .0
            .pre_translate(lyon::math::Vector::new(translation.x, translation.y));
    }

    #[inline]
    fn rotate(&mut self, angle: impl Into<Radians>) {
        self.transforms.current.0 = self
            .transforms
            .current
            .0
            .pre_rotate(lyon::math::Angle::radians(angle.into().0));
    }

    #[inline]
    fn scale(&mut self, scale: f32) {
        self.scale_nonuniform(Vector { x: scale, y: scale });
    }

    #[inline]
    fn scale_nonuniform(&mut self, scale: impl Into<Vector>) {
        let scale = scale.into();

        self.transforms.current.0 = self.transforms.current.0.pre_scale(scale.x, scale.y);
    }

    fn push_transform(&mut self) {
        self.transforms.previous.push(self.transforms.current);
    }

    fn pop_transform(&mut self) {
        self.transforms.current = self.transforms.previous.pop().unwrap();
    }

    fn draft(&mut self, clip_bounds: Rectangle) -> Frame {
        Frame::new(clip_bounds)
    }

    fn paste(&mut self, frame: Frame) {
        for section in frame.into_sections() {
            let Section {
                meshes,
                primitives,
                images,
                text,
            } = section;

            if !meshes.is_empty() {
                self.begin(Kind::Shape);

                // Whatever has been drawn in this frame so far must stay
                // below the pasted meshes.
                self.seal();

                self.sections.current_mut().meshes.extend(meshes);
            }

            if !primitives.is_empty() {
                self.begin(Kind::Primitive);
                self.sections.current_mut().primitives.extend(primitives);
            }

            if !images.is_empty() {
                self.begin(Kind::Image);
                self.sections.current_mut().images.extend(images);
            }

            if !text.is_empty() {
                self.begin(Kind::Text);
                self.sections.current_mut().text.extend(text);
            }
        }
    }

    fn into_geometry(self) -> Self::Geometry {
        Geometry::Live {
            sections: self.into_sections(),
        }
    }

    fn draw_image(&mut self, bounds: Rectangle, image: impl Into<core::Image>) {
        let mut image = image.into();

        let (bounds, external_rotation) = self.transforms.current.transform_rectangle(bounds);

        image.rotation += external_rotation;
        image.border_radius = image.border_radius * self.transforms.current.scale().0;

        self.begin(Kind::Image);

        let clip_bounds = self.clip_bounds;

        self.sections.current_mut().images.push(Image::Raster {
            image,
            bounds,
            clip_bounds,
        });
    }

    fn draw_svg(&mut self, bounds: Rectangle, svg: impl Into<Svg>) {
        let mut svg = svg.into();

        let (bounds, external_rotation) = self.transforms.current.transform_rectangle(bounds);

        svg.rotation += external_rotation;

        self.begin(Kind::Image);

        let clip_bounds = self.clip_bounds;

        self.sections.current_mut().images.push(Image::Vector {
            svg,
            bounds,
            clip_bounds,
        });
    }
}

impl<P: crate::Primitive> geometry::frame::DrawPrimitive<P> for Frame {
    fn draw_primitive(&mut self, bounds: Rectangle, primitive: P) {
        // A primitive cannot be rotated, so it covers the axis-aligned
        // bounding box of the transformed rectangle (which also handles
        // mirrored transforms, unlike `transform_rectangle`).
        let bounds = self.transforms.current.bounding_box(bounds);

        // Nothing to draw if the primitive is clipped away entirely.
        let Some(instance) = primitive::Instance::new(bounds, primitive).clipped(self.clip_bounds)
        else {
            return;
        };

        self.begin(Kind::Primitive);

        self.sections.current_mut().primitives.push(instance);
    }
}

enum Buffer {
    Solid(tessellation::VertexBuffers<mesh::SolidVertex2D, u32>),
    Gradient(tessellation::VertexBuffers<mesh::GradientVertex2D, u32>),
}

struct BufferStack {
    stack: Vec<Buffer>,
}

impl BufferStack {
    fn new() -> Self {
        Self { stack: Vec::new() }
    }

    fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    fn get_mut(&mut self, style: &Style) -> &mut Buffer {
        match style {
            Style::Solid(_) => match self.stack.last() {
                Some(Buffer::Solid(_)) => {}
                _ => {
                    self.stack
                        .push(Buffer::Solid(tessellation::VertexBuffers::new()));
                }
            },
            Style::Gradient(_) => match self.stack.last() {
                Some(Buffer::Gradient(_)) => {}
                _ => {
                    self.stack
                        .push(Buffer::Gradient(tessellation::VertexBuffers::new()));
                }
            },
        }

        self.stack.last_mut().unwrap()
    }

    fn get_fill<'a>(
        &'a mut self,
        style: &Style,
    ) -> Box<dyn tessellation::FillGeometryBuilder + 'a> {
        match (style, self.get_mut(style)) {
            (Style::Solid(color), Buffer::Solid(buffer)) => {
                Box::new(tessellation::BuffersBuilder::new(
                    buffer,
                    TriangleVertex2DBuilder(color::pack(*color)),
                ))
            }
            (Style::Gradient(gradient), Buffer::Gradient(buffer)) => {
                Box::new(tessellation::BuffersBuilder::new(
                    buffer,
                    GradientVertex2DBuilder {
                        gradient: gradient.pack(),
                    },
                ))
            }
            _ => unreachable!(),
        }
    }

    fn get_stroke<'a>(
        &'a mut self,
        style: &Style,
    ) -> Box<dyn tessellation::StrokeGeometryBuilder + 'a> {
        match (style, self.get_mut(style)) {
            (Style::Solid(color), Buffer::Solid(buffer)) => {
                Box::new(tessellation::BuffersBuilder::new(
                    buffer,
                    TriangleVertex2DBuilder(color::pack(*color)),
                ))
            }
            (Style::Gradient(gradient), Buffer::Gradient(buffer)) => {
                Box::new(tessellation::BuffersBuilder::new(
                    buffer,
                    GradientVertex2DBuilder {
                        gradient: gradient.pack(),
                    },
                ))
            }
            _ => unreachable!(),
        }
    }

    fn into_meshes(self, clip_bounds: Rectangle) -> impl Iterator<Item = Mesh> {
        self.stack
            .into_iter()
            .filter_map(move |buffer| match buffer {
                Buffer::Solid(buffer) if !buffer.indices.is_empty() => Some(Mesh::Solid {
                    buffers: mesh::Indexed {
                        vertices: buffer.vertices,
                        indices: buffer.indices,
                    },
                    clip_bounds,
                    transformation: Transformation::IDENTITY,
                }),
                Buffer::Gradient(buffer) if !buffer.indices.is_empty() => Some(Mesh::Gradient {
                    buffers: mesh::Indexed {
                        vertices: buffer.vertices,
                        indices: buffer.indices,
                    },
                    clip_bounds,
                    transformation: Transformation::IDENTITY,
                }),
                _ => None,
            })
    }
}

#[derive(Debug)]
struct Transforms {
    previous: Vec<Transform>,
    current: Transform,
}

#[derive(Debug, Clone, Copy)]
struct Transform(lyon::math::Transform);

impl Transform {
    fn is_identity(&self) -> bool {
        self.0 == lyon::math::Transform::identity()
    }

    fn is_scale_translation(&self) -> bool {
        self.0.m12.abs() < 2.0 * f32::EPSILON && self.0.m21.abs() < 2.0 * f32::EPSILON
    }

    fn scale(&self) -> (f32, f32) {
        (self.0.m11, self.0.m22)
    }

    fn transform_point(&self, point: Point) -> Point {
        let transformed = self
            .0
            .transform_point(euclid::Point2D::new(point.x, point.y));

        Point {
            x: transformed.x,
            y: transformed.y,
        }
    }

    fn transform_style(&self, style: Style) -> Style {
        match style {
            Style::Solid(color) => Style::Solid(color),
            Style::Gradient(gradient) => Style::Gradient(self.transform_gradient(gradient)),
        }
    }

    fn transform_gradient(&self, mut gradient: Gradient) -> Gradient {
        match &mut gradient {
            Gradient::Linear(linear) => {
                linear.start = self.transform_point(linear.start);
                linear.end = self.transform_point(linear.end);
            }
        }

        gradient
    }

    /// The axis-aligned bounding box of the transformed `rectangle`.
    fn bounding_box(&self, rectangle: Rectangle) -> Rectangle {
        let corners = [
            Vector::new(0.0, 0.0),
            Vector::new(rectangle.width, 0.0),
            Vector::new(0.0, rectangle.height),
            Vector::new(rectangle.width, rectangle.height),
        ]
        .map(|corner| self.transform_point(rectangle.position() + corner));

        let (min_x, max_x) = corners.iter().fold((f32::MAX, f32::MIN), |(min, max), p| {
            (min.min(p.x), max.max(p.x))
        });
        let (min_y, max_y) = corners.iter().fold((f32::MAX, f32::MIN), |(min, max), p| {
            (min.min(p.y), max.max(p.y))
        });

        Rectangle::new(
            Point::new(min_x, min_y),
            Size::new(max_x - min_x, max_y - min_y),
        )
    }

    fn transform_rectangle(&self, rectangle: Rectangle) -> (Rectangle, Radians) {
        let top_left = self.transform_point(rectangle.position());
        let top_right =
            self.transform_point(rectangle.position() + Vector::new(rectangle.width, 0.0));
        let bottom_left =
            self.transform_point(rectangle.position() + Vector::new(0.0, rectangle.height));

        Rectangle::with_vertices(top_left, top_right, bottom_left)
    }
}
struct GradientVertex2DBuilder {
    gradient: gradient::Packed,
}

impl tessellation::FillVertexConstructor<mesh::GradientVertex2D> for GradientVertex2DBuilder {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex<'_>) -> mesh::GradientVertex2D {
        let position = vertex.position();

        mesh::GradientVertex2D {
            position: [position.x, position.y],
            gradient: self.gradient,
        }
    }
}

impl tessellation::StrokeVertexConstructor<mesh::GradientVertex2D> for GradientVertex2DBuilder {
    fn new_vertex(&mut self, vertex: tessellation::StrokeVertex<'_, '_>) -> mesh::GradientVertex2D {
        let position = vertex.position();

        mesh::GradientVertex2D {
            position: [position.x, position.y],
            gradient: self.gradient,
        }
    }
}

struct TriangleVertex2DBuilder(color::Packed);

impl tessellation::FillVertexConstructor<mesh::SolidVertex2D> for TriangleVertex2DBuilder {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex<'_>) -> mesh::SolidVertex2D {
        let position = vertex.position();

        mesh::SolidVertex2D {
            position: [position.x, position.y],
            color: self.0,
        }
    }
}

impl tessellation::StrokeVertexConstructor<mesh::SolidVertex2D> for TriangleVertex2DBuilder {
    fn new_vertex(&mut self, vertex: tessellation::StrokeVertex<'_, '_>) -> mesh::SolidVertex2D {
        let position = vertex.position();

        mesh::SolidVertex2D {
            position: [position.x, position.y],
            color: self.0,
        }
    }
}

fn into_line_join(line_join: LineJoin) -> lyon::tessellation::LineJoin {
    match line_join {
        LineJoin::Miter => lyon::tessellation::LineJoin::Miter,
        LineJoin::Round => lyon::tessellation::LineJoin::Round,
        LineJoin::Bevel => lyon::tessellation::LineJoin::Bevel,
    }
}

fn into_line_cap(line_cap: LineCap) -> lyon::tessellation::LineCap {
    match line_cap {
        LineCap::Butt => lyon::tessellation::LineCap::Butt,
        LineCap::Square => lyon::tessellation::LineCap::Square,
        LineCap::Round => lyon::tessellation::LineCap::Round,
    }
}

fn into_fill_rule(rule: fill::Rule) -> lyon::tessellation::FillRule {
    match rule {
        fill::Rule::NonZero => lyon::tessellation::FillRule::NonZero,
        fill::Rule::EvenOdd => lyon::tessellation::FillRule::EvenOdd,
    }
}

pub(super) fn dashed(path: &Path, line_dash: LineDash<'_>) -> Path {
    use lyon::algorithms::walk::{RepeatedPattern, WalkerEvent, walk_along_path};
    use lyon::path::iterator::PathIterator;

    Path::new(|builder| {
        let segments_odd = (line_dash.segments.len() % 2 == 1)
            .then(|| [line_dash.segments, line_dash.segments].concat());

        let mut draw_line = false;

        walk_along_path(
            path.raw()
                .iter()
                .flattened(lyon::tessellation::StrokeOptions::DEFAULT_TOLERANCE),
            0.0,
            lyon::tessellation::StrokeOptions::DEFAULT_TOLERANCE,
            &mut RepeatedPattern {
                callback: |event: WalkerEvent<'_>| {
                    let point = Point {
                        x: event.position.x,
                        y: event.position.y,
                    };

                    if draw_line {
                        builder.line_to(point);
                    } else {
                        builder.move_to(point);
                    }

                    draw_line = !draw_line;

                    true
                },
                index: line_dash.offset,
                intervals: segments_odd.as_deref().unwrap_or(line_dash.segments),
            },
        );
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Color;
    use crate::graphics::geometry::frame::{Backend as _, DrawPrimitive};

    /// (meshes, atlas text runs) a frame holds after drawing `"R1"` at `size`.
    fn drawn(size: f32) -> (usize, usize) {
        let mut frame = Frame::new(Rectangle::with_size(Size::new(8000.0, 8000.0)));
        frame.fill_text(geometry::Text {
            content: "R1".into(),
            size: Pixels(size),
            ..geometry::Text::default()
        });
        let Geometry::Live { sections } = frame.into_geometry() else {
            unreachable!("a new frame is live");
        };
        (
            sections.iter().map(|s| s.meshes.len()).sum(),
            sections.iter().map(|s| s.text.len()).sum(),
        )
    }

    #[test]
    fn text_past_the_atlas_limit_is_meshes_and_ordinary_text_is_atlas_text() {
        let (meshes, text) = drawn(2000.0);
        assert!(meshes > 0 && text == 0, "{meshes} meshes, {text} text");
        let (meshes, text) = drawn(16.0);
        assert!(meshes == 0 && text > 0, "{meshes} meshes, {text} text");
    }

    /// A primitive that paints its bounds solid green.
    #[derive(Debug)]
    struct Green;

    struct GreenPipeline(wgpu::RenderPipeline);

    impl crate::primitive::Pipeline for GreenPipeline {
        fn new(device: &wgpu::Device, _queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("green"),
                source: wgpu::ShaderSource::Wgsl(
                    "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                        let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
                        return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
                    }
                    @fragment fn fs() -> @location(0) vec4<f32> {
                        return vec4<f32>(0.0, 1.0, 0.0, 1.0);
                    }"
                    .into(),
                ),
            });

            GreenPipeline(
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("green"),
                    layout: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs"),
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                        targets: &[Some(format.into())],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                }),
            )
        }
    }

    impl crate::Primitive for Green {
        type Pipeline = GreenPipeline;

        fn prepare(
            &self,
            _pipeline: &mut GreenPipeline,
            _device: &wgpu::Device,
            _queue: &wgpu::Queue,
            _bounds: &Rectangle,
            _viewport: &crate::graphics::Viewport,
        ) {
        }

        fn draw(&self, pipeline: &GreenPipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
            render_pass.set_pipeline(&pipeline.0);
            render_pass.draw(0..3, 0..1);

            true
        }
    }

    fn rectangle(frame: &mut Frame, x: f32, y: f32, color: Color) {
        frame.fill_rectangle(Point::new(x, y), Size::new(40.0, 40.0), color);
    }

    #[test]
    fn primitives_split_sections_in_draw_order() {
        let bounds = Rectangle::new(Point::ORIGIN, Size::new(10.0, 10.0));
        let mut frame = Frame::new(bounds);

        rectangle(&mut frame, 0.0, 0.0, Color::BLACK);
        DrawPrimitive::draw_primitive(&mut frame, bounds, Green);
        DrawPrimitive::draw_primitive(&mut frame, bounds, Green);
        rectangle(&mut frame, 0.0, 0.0, Color::BLACK);

        let Geometry::Live { sections } = frame.into_geometry() else {
            unreachable!("a new frame is live");
        };

        // Shapes, then primitives, then shapes again cannot share a layer.
        let counts: Vec<_> = sections
            .iter()
            .map(|s| (s.meshes.len(), s.primitives.len()))
            .collect();

        assert_eq!(counts, vec![(1, 2), (1, 0)]);
    }

    /// Renders `geometry` offscreen and returns the RGBA pixel at `(x, y)`
    /// of a 100x100 image, or `None` if there is no GPU adapter.
    fn render(geometry: impl Fn() -> Geometry) -> Option<impl Fn(usize, usize) -> [u8; 4]> {
        use crate::graphics::geometry::Renderer as _;

        let mut renderer =
            futures::executor::block_on(<crate::Renderer as crate::renderer::Headless>::new(
                core::renderer::Settings::default(),
                None,
            ))?;

        renderer.draw_geometry(geometry());

        let size = Size::new(100, 100);
        let pixels = renderer.screenshot(
            &crate::graphics::Viewport::with_physical_size(
                size,
                core::renderer::Scale {
                    window: 1.0,
                    application: 1.0,
                },
            ),
            Color::TRANSPARENT,
        );

        Some(move |x: usize, y: usize| {
            let i = (y * 100 + x) * 4;
            [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
        })
    }

    /// Reports a GPU test that could not run. Set `ICED_REQUIRE_GPU_TESTS` to
    /// make it fail instead of passing silently.
    fn skipped() {
        assert!(
            std::env::var_os("ICED_REQUIRE_GPU_TESTS").is_none(),
            "no GPU adapter available, but ICED_REQUIRE_GPU_TESTS is set"
        );

        eprintln!("SKIPPED: no GPU adapter available (set ICED_REQUIRE_GPU_TESTS=1 to fail)");
    }

    /// A red square, a green primitive on top of it, and a blue square on top
    /// of the primitive, all drawn in a single frame.
    fn sandwich() -> Geometry {
        let mut frame = Frame::new(Rectangle::with_size(Size::new(100.0, 100.0)));

        rectangle(&mut frame, 10.0, 10.0, Color::from_rgb(1.0, 0.0, 0.0));
        DrawPrimitive::draw_primitive(
            &mut frame,
            Rectangle::new(Point::new(30.0, 30.0), Size::new(40.0, 40.0)),
            Green,
        );
        rectangle(&mut frame, 50.0, 50.0, Color::from_rgb(0.0, 0.0, 1.0));

        frame.into_geometry()
    }

    const RED: [u8; 4] = [255, 0, 0, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    #[test]
    fn primitive_renders_between_geometry() {
        let Some(pixel) = render(sandwich) else {
            return skipped();
        };

        assert_eq!(pixel(15, 15), RED, "red only");
        assert_eq!(
            pixel(35, 35),
            GREEN,
            "the primitive is above earlier shapes"
        );
        assert_eq!(pixel(65, 65), BLUE, "later shapes are above the primitive");
        assert_eq!(pixel(55, 35), GREEN, "primitive only");
        assert_eq!(pixel(95, 95), [0, 0, 0, 0], "background");
    }

    #[test]
    fn cached_primitive_renders_between_geometry() {
        let cached = || {
            let cache = sandwich().cache(cache::Group::unique(), None);

            Geometry::load(&cache)
        };

        let Some(pixel) = render(cached) else {
            return skipped();
        };

        assert_eq!(pixel(15, 15), RED, "red only");
        assert_eq!(
            pixel(35, 35),
            GREEN,
            "the primitive is above earlier shapes"
        );
        assert_eq!(pixel(65, 65), BLUE, "later shapes are above the primitive");
    }

    /// Geometry of a frame of the given size, filled in by `draw`.
    fn primitive_frame(frame_size: Size, draw: impl FnOnce(&mut Frame)) -> impl Fn() -> Geometry {
        let mut frame = Frame::new(Rectangle::with_size(frame_size));
        draw(&mut frame);
        let geometry = std::cell::RefCell::new(Some(frame.into_geometry()));

        move || geometry.borrow_mut().take().expect("rendered once")
    }

    const NONE: [u8; 4] = [0, 0, 0, 0];

    #[test]
    fn primitive_is_clipped_to_the_frame() {
        let geometry = primitive_frame(Size::new(50.0, 50.0), |frame| {
            DrawPrimitive::draw_primitive(
                frame,
                Rectangle::new(Point::new(30.0, 30.0), Size::new(60.0, 60.0)),
                Green,
            );
        });

        let Some(pixel) = render(geometry) else {
            return skipped();
        };

        assert_eq!(pixel(40, 40), GREEN, "inside the frame");
        assert_eq!(pixel(60, 60), NONE, "past the frame");
        assert_eq!(pixel(85, 85), NONE, "past the frame, inside the bounds");
    }

    #[test]
    fn primitive_is_clipped_to_with_clip_region() {
        let geometry = primitive_frame(Size::new(100.0, 100.0), |frame| {
            let mut clipped = frame.draft(Rectangle::new(
                Point::new(20.0, 20.0),
                Size::new(40.0, 40.0),
            ));

            DrawPrimitive::draw_primitive(
                &mut clipped,
                Rectangle::new(Point::new(10.0, 10.0), Size::new(80.0, 80.0)),
                Green,
            );

            frame.paste(clipped);
        });

        let Some(pixel) = render(geometry) else {
            return skipped();
        };

        assert_eq!(pixel(15, 15), NONE, "before the region");
        assert_eq!(pixel(30, 30), GREEN, "inside the region");
        assert_eq!(pixel(70, 70), NONE, "after the region");
    }

    #[test]
    fn primitive_follows_a_flipped_transform() {
        let geometry = primitive_frame(Size::new(100.0, 100.0), |frame| {
            frame.translate(Vector::new(0.0, 80.0));
            frame.scale_nonuniform(Vector::new(1.0, -1.0));

            // Y-up: covers x in [10, 40] and y in [30, 70] on screen.
            DrawPrimitive::draw_primitive(
                frame,
                Rectangle::new(Point::new(10.0, 10.0), Size::new(30.0, 40.0)),
                Green,
            );
        });

        let Some(pixel) = render(geometry) else {
            return skipped();
        };

        assert_eq!(pixel(20, 50), GREEN, "where the flipped rectangle is");
        assert_eq!(pixel(20, 20), NONE, "above it");
        assert_eq!(pixel(20, 75), NONE, "below it");
        assert_eq!(pixel(60, 50), NONE, "beside it");
    }

    #[test]
    fn primitive_bounds_are_the_bounding_box_of_a_rotation() {
        let mut frame = Frame::new(Rectangle::with_size(Size::new(100.0, 100.0)));

        frame.translate(Vector::new(50.0, 50.0));
        frame.rotate(Radians(std::f32::consts::FRAC_PI_2));

        // A 20x10 rectangle turned a quarter turn is 10x20.
        DrawPrimitive::draw_primitive(
            &mut frame,
            Rectangle::new(Point::ORIGIN, Size::new(20.0, 10.0)),
            Green,
        );

        let Geometry::Live { sections } = frame.into_geometry() else {
            unreachable!("a new frame is live");
        };

        let bounds = sections[0].primitives[0].bounds;

        assert!((bounds.x - 40.0).abs() < 1e-3, "{bounds:?}");
        assert!((bounds.y - 50.0).abs() < 1e-3, "{bounds:?}");
        assert!((bounds.width - 10.0).abs() < 1e-3, "{bounds:?}");
        assert!((bounds.height - 20.0).abs() < 1e-3, "{bounds:?}");
    }

    #[test]
    fn primitive_outside_the_frame_is_not_drawn() {
        let mut frame = Frame::new(Rectangle::with_size(Size::new(50.0, 50.0)));

        DrawPrimitive::draw_primitive(
            &mut frame,
            Rectangle::new(Point::new(200.0, 200.0), Size::new(40.0, 40.0)),
            Green,
        );

        let Geometry::Live { sections } = frame.into_geometry() else {
            unreachable!("a new frame is live");
        };

        assert!(sections.iter().all(|s| s.primitives.is_empty()));
    }

    #[test]
    fn primitive_clip_is_inside_its_bounds() {
        let mut frame = Frame::new(Rectangle::with_size(Size::new(50.0, 50.0)));

        DrawPrimitive::draw_primitive(
            &mut frame,
            Rectangle::new(Point::new(30.0, 30.0), Size::new(60.0, 60.0)),
            Green,
        );

        let Geometry::Live { sections } = frame.into_geometry() else {
            unreachable!("a new frame is live");
        };

        let instance = &sections[0].primitives[0];

        assert_eq!(
            instance.clip_bounds,
            Rectangle::new(Point::new(30.0, 30.0), Size::new(20.0, 20.0))
        );
        assert_eq!(instance.bounds.width, 60.0);
    }

    #[test]
    fn primitive_far_larger_than_the_device_limits_does_not_panic() {
        let geometry = primitive_frame(Size::new(100.0, 100.0), |frame| {
            DrawPrimitive::draw_primitive(
                frame,
                Rectangle::new(
                    Point::new(-200_000.0, -200_000.0),
                    Size::new(400_000.0, 400_000.0),
                ),
                Green,
            );
        });

        let Some(pixel) = render(geometry) else {
            return skipped();
        };

        assert_eq!(pixel(50, 50), NONE, "skipped, not stretched or crashed");
    }

    /// A [`Green`] that records whether it was prepared.
    #[derive(Debug)]
    struct Probe(std::sync::Arc<std::sync::atomic::AtomicBool>);

    impl crate::Primitive for Probe {
        type Pipeline = GreenPipeline;

        fn prepare(
            &self,
            _pipeline: &mut GreenPipeline,
            _device: &wgpu::Device,
            _queue: &wgpu::Queue,
            _bounds: &Rectangle,
            _viewport: &crate::graphics::Viewport,
        ) {
            self.0.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        fn draw(&self, pipeline: &GreenPipeline, render_pass: &mut wgpu::RenderPass<'_>) -> bool {
            crate::Primitive::draw(&Green, pipeline, render_pass)
        }
    }

    #[test]
    fn primitive_far_larger_than_the_device_limits_is_not_prepared() {
        let prepared = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let probe = Probe(prepared.clone());

        let geometry = primitive_frame(Size::new(100.0, 100.0), move |frame| {
            DrawPrimitive::draw_primitive(
                frame,
                Rectangle::new(
                    Point::new(-200_000.0, -200_000.0),
                    Size::new(400_000.0, 400_000.0),
                ),
                probe,
            );
        });

        if render(geometry).is_none() {
            return skipped();
        }

        assert!(
            !prepared.load(std::sync::atomic::Ordering::Relaxed),
            "prepare must be skipped for a primitive the render pass skips"
        );
    }
}
