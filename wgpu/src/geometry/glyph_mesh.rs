//! Glyph outlines tessellated once, in glyph-local coordinates, and placed
//! through the frame transform on every later draw.
//!
//! Text that cannot use the glyph atlas (rotated, skewed, mirrored or too big)
//! is filled as vector outlines. Tessellating every outline on every frame is
//! what made such text cost a fraction of a millisecond per label; with this
//! cache the tessellation is done once. Each frame still lays the text out
//! (shaping), then transforms the cached vertices.
use crate::graphics::geometry::GlyphKey;
use crate::graphics::geometry::GlyphOutline;

use lyon::math::Transform;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
};

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A glyph mesh in glyph-local coordinates.
pub(super) type LocalMesh = VertexBuffers<[f32; 2], u32>;

/// How many tolerance buckets one doubling of the device scale spans.
const BUCKETS_PER_OCTAVE: f32 = 4.0;

/// The most meshes the cache holds before it is emptied; it would otherwise
/// grow with every glyph, size and zoom level ever drawn as outlines.
const MAX_MESHES: usize = 16 * 1024;

/// The most mesh data, in bytes, the cache holds before it is emptied. Large
/// glyphs at high zoom have far more vertices than small ones, so the entry
/// count alone does not bound memory.
const MAX_MESH_BYTES: usize = 64 * 1024 * 1024;

/// The largest tolerance bucket. Beyond it the glyph-local tolerance nears
/// lyon's `f32` epsilon-squared debug assertion; such scales are far past any
/// real zoom.
const MAX_BUCKET: i32 = 160;

#[derive(Default)]
struct Cache {
    meshes: HashMap<(GlyphKey, i32), Rc<LocalMesh>>,
    /// The bytes of mesh data in `meshes`.
    bytes: usize,
}

impl Cache {
    fn insert(&mut self, key: (GlyphKey, i32), mesh: Rc<LocalMesh>) {
        let bytes = size_of_mesh(&mesh);

        if self.meshes.len() >= MAX_MESHES || self.bytes + bytes > MAX_MESH_BYTES {
            // Replace rather than `clear`, so the table's memory goes too.
            *self = Cache::default();
        }

        self.bytes += bytes;
        let _ = self.meshes.insert(key, mesh);
    }
}

fn size_of_mesh(mesh: &LocalMesh) -> usize {
    mesh.vertices.len() * size_of::<[f32; 2]>() + mesh.indices.len() * size_of::<u32>()
}

thread_local! {
    static MESHES: RefCell<Cache> = RefCell::new(Cache::default());
}

/// The tolerance bucket of a frame transform: its largest stretch rounded up
/// to a quarter octave, so one mesh serves every transform in the bucket.
/// `None` when the transform collapses entirely, is not finite, or stretches
/// past `MAX_BUCKET`.
pub(super) fn bucket(transform: &Transform) -> Option<i32> {
    // The largest singular value of the linear part is its largest stretch,
    // so a mesh flattened for it is fine enough in every direction. This
    // closed form has no cancellation, unlike the root of the characteristic
    // polynomial.
    let (a, b, c, d) = (transform.m11, transform.m12, transform.m21, transform.m22);
    let scale = (f32::hypot(a + d, b - c) + f32::hypot(a - d, b + c)) / 2.0;

    (scale.is_finite() && scale > 0.0)
        .then(|| (scale.log2() * BUCKETS_PER_OCTAVE).ceil() as i32)
        .filter(|&bucket| bucket <= MAX_BUCKET)
}

/// The mesh of `outline` for transforms in `bucket`, tessellated on the first
/// request and shared after. `None` if the outline cannot be tessellated.
pub(super) fn mesh(
    outline: &GlyphOutline<'_>,
    bucket: i32,
    tessellator: &mut FillTessellator,
) -> Option<Rc<LocalMesh>> {
    let key = (outline.key, bucket);

    let cached = MESHES.try_with(|cache| cache.borrow().meshes.get(&key).map(Rc::clone));

    // An `Err` is thread-local teardown: the mesh is then not cached.
    if let Ok(Some(mesh)) = cached {
        return Some(mesh);
    }

    let mesh = Rc::new(tessellate(outline, bucket, tessellator)?);

    let _ = MESHES.try_with(|cache| cache.borrow_mut().insert(key, Rc::clone(&mesh)));

    Some(mesh)
}

/// The device-space tolerance of every canvas fill, divided by the largest
/// scale in the bucket.
fn local_tolerance(bucket: i32) -> f32 {
    FillOptions::DEFAULT_TOLERANCE / (bucket as f32 / BUCKETS_PER_OCTAVE).exp2()
}

fn tessellate(
    outline: &GlyphOutline<'_>,
    bucket: i32,
    tessellator: &mut FillTessellator,
) -> Option<LocalMesh> {
    // Non-zero, as `Frame::fill` fills a path given a plain color.
    let options = FillOptions::tolerance(local_tolerance(bucket)).with_fill_rule(FillRule::NonZero);

    let mut mesh = LocalMesh::new();

    tessellator
        .tessellate_path(
            outline.local_path().raw(),
            &options,
            &mut BuffersBuilder::new(&mut mesh, |vertex: FillVertex<'_>| {
                vertex.position().to_array()
            }),
        )
        .ok()?;

    Some(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Pixels, Point, Radians, Rectangle, Size, Vector};
    use crate::geometry::{Frame, Geometry};
    use crate::graphics::geometry::{self, frame::Backend as _};
    use crate::graphics::mesh::Mesh;

    fn cached() -> usize {
        MESHES.with_borrow(|cache| cache.meshes.len())
    }

    fn label() -> geometry::Text {
        geometry::Text {
            content: "R12 Vout".into(),
            position: Point::new(40.0, 30.0),
            size: Pixels(14.0),
            ..geometry::Text::default()
        }
    }

    /// A frame with `transform` applied, as a closure on a fresh frame.
    fn frame(transform: impl Fn(&mut Frame)) -> Frame {
        let mut frame = Frame::new(Rectangle::with_size(Size::new(2000.0, 2000.0)));
        transform(&mut frame);
        frame
    }

    /// The triangles a frame holds: total area and bounding box.
    fn triangles(frame: Frame) -> (f32, Rectangle) {
        let Geometry::Live { sections } = frame.into_geometry() else {
            unreachable!("a new frame is live");
        };

        let (mut area, mut min, mut max) = (0.0, [f32::MAX; 2], [f32::MIN; 2]);

        for mesh in sections.iter().flat_map(|s| &s.meshes) {
            let Mesh::Solid { buffers, .. } = mesh else {
                panic!("glyphs are solid meshes");
            };
            let p = |i: u32| buffers.vertices[i as usize].position;

            for t in buffers.indices.chunks(3) {
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                area += ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs() / 2.0;

                for v in [a, b, c] {
                    min = [min[0].min(v[0]), min[1].min(v[1])];
                    max = [max[0].max(v[0]), max[1].max(v[1])];
                }
            }
        }

        let bounds = Rectangle::new(
            Point::new(min[0], min[1]),
            Size::new(max[0] - min[0], max[1] - min[1]),
        );

        (area, bounds)
    }

    /// The text drawn from the cache against the text tessellated as paths,
    /// the way every frame did before the cache.
    fn cached_and_tessellated(transform: impl Fn(&mut Frame)) -> [(f32, Rectangle); 2] {
        let mut cached = frame(&transform);
        cached.fill_text(label());

        let mut tessellated = frame(&transform);
        label().draw_with(|path, color| tessellated.fill(&path, color));

        [triangles(cached), triangles(tessellated)]
    }

    fn assert_same_glyphs(transform: impl Fn(&mut Frame)) {
        let [(area, bounds), (expected_area, expected_bounds)] = cached_and_tessellated(transform);

        assert!(expected_area > 0.0, "the label draws nothing");
        assert!(
            (area - expected_area).abs() <= expected_area * 0.01,
            "area {area} against {expected_area}"
        );

        for (got, expected) in [
            (bounds.x, expected_bounds.x),
            (bounds.y, expected_bounds.y),
            (bounds.width, expected_bounds.width),
            (bounds.height, expected_bounds.height),
        ] {
            // Two flattenings of one curve, each within the 0.1 px tolerance.
            assert!(
                (got - expected).abs() <= 0.25,
                "{bounds:?} against {expected_bounds:?}"
            );
        }
    }

    #[test]
    fn a_rotated_label_from_the_cache_matches_one_tessellated_each_frame() {
        assert_same_glyphs(|frame| {
            frame.translate(Vector::new(500.0, 400.0));
            frame.rotate(Radians(std::f32::consts::FRAC_PI_2));
        });
    }

    #[test]
    fn a_zoomed_odd_angle_label_from_the_cache_matches_one_tessellated_each_frame() {
        assert_same_glyphs(|frame| {
            frame.translate(Vector::new(300.0, 900.0));
            frame.rotate(Radians(0.6));
            frame.scale(5.3);
        });
    }

    #[test]
    fn an_anisotropic_label_from_the_cache_matches_one_tessellated_each_frame() {
        assert_same_glyphs(|frame| {
            frame.translate(Vector::new(200.0, 700.0));
            frame.rotate(Radians(0.4));
            frame.scale_nonuniform(Vector::new(3.0, 1.0));
            frame.rotate(Radians(0.9));
        });
    }

    #[test]
    fn a_rotated_label_keeps_its_color() {
        let color = crate::core::Color::from_rgb(0.2, 0.6, 0.9);
        let mut rotated = frame(|frame| frame.rotate(Radians(std::f32::consts::FRAC_PI_2)));
        rotated.fill_text(geometry::Text { color, ..label() });

        let Geometry::Live { sections } = rotated.into_geometry() else {
            unreachable!("a new frame is live");
        };
        let packed = crate::graphics::color::pack(color);
        let mut vertices = 0;

        for mesh in sections.iter().flat_map(|s| &s.meshes) {
            let Mesh::Solid { buffers, .. } = mesh else {
                panic!("glyphs are solid meshes");
            };
            vertices += buffers.vertices.len();
            assert!(buffers.vertices.iter().all(|v| v.color == packed));
        }

        assert!(vertices > 0, "the label draws nothing");
    }

    #[test]
    fn glyphs_after_atlas_text_start_a_new_section() {
        let mut frame = frame(|_| {});
        frame.fill_text(geometry::Text {
            size: Pixels(16.0),
            ..label()
        });
        frame.rotate(Radians(std::f32::consts::FRAC_PI_2));
        frame.fill_text(label());

        let Geometry::Live { sections } = frame.into_geometry() else {
            unreachable!("a new frame is live");
        };

        let counts: Vec<_> = sections
            .iter()
            .map(|s| (!s.text.is_empty(), !s.meshes.is_empty()))
            .collect();

        assert_eq!(counts, vec![(true, false), (false, true)]);
    }

    #[test]
    fn a_mirrored_label_from_the_cache_matches_one_tessellated_each_frame() {
        assert_same_glyphs(|frame| {
            frame.translate(Vector::new(800.0, 200.0));
            frame.scale_nonuniform(Vector::new(-2.0, 2.0));
        });
    }

    #[test]
    fn drawing_a_rotated_label_again_adds_no_meshes() {
        let draw = || {
            let mut rotated = frame(|frame| frame.rotate(Radians(std::f32::consts::FRAC_PI_2)));
            rotated.fill_text(label());
            triangles(rotated)
        };

        let first = draw();
        let meshes = cached();
        assert!(meshes > 0, "the first draw keeps its glyph meshes");

        assert_eq!(draw(), first);
        assert_eq!(cached(), meshes, "the second draw tessellated");
    }

    #[test]
    fn transforms_share_a_mesh_within_a_quarter_octave_and_not_across_one() {
        let scaled = |scale: f32| bucket(&Transform::scale(scale, scale));

        assert_eq!(scaled(1.0), scaled(1.0));
        assert_eq!(scaled(1.05), scaled(1.15));
        assert_ne!(scaled(1.0), scaled(1.3));
        assert_ne!(scaled(1.0), scaled(0.5));
        assert_eq!(
            bucket(&Transform::rotation(lyon::math::Angle::radians(0.7))),
            bucket(&Transform::rotation(lyon::math::Angle::radians(2.1))),
            "rotation alone does not change the bucket"
        );
        assert_eq!(
            bucket(&Transform::rotation(lyon::math::Angle::radians(0.7))),
            Some(0),
            "a pure rotation buckets like the identity"
        );

        for angle in [
            0.0,
            0.3,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
            4.0,
        ] {
            let angle = lyon::math::Angle::radians(angle);

            assert_eq!(bucket(&Transform::rotation(angle)), Some(0));
            assert_eq!(
                bucket(&Transform::scale(2.0, 2.0).then_rotate(angle)),
                Some(4)
            );
        }
    }

    #[test]
    fn a_second_request_returns_the_same_mesh() {
        let mut tessellator = FillTessellator::new();
        let mut checked = false;

        label().visit_glyphs(|glyph| {
            let geometry::Glyph::Outline(outline) = glyph else {
                return;
            };
            if checked {
                return;
            }
            checked = true;

            let first = mesh(&outline, 0, &mut tessellator).expect("the outline tessellates");
            let second = mesh(&outline, 0, &mut tessellator).expect("the outline tessellates");

            assert!(
                Rc::ptr_eq(&first, &second),
                "the second request tessellated"
            );
        });

        assert!(checked, "the label has an outline");
    }

    #[test]
    fn a_label_drawn_zoomed_out_then_zoomed_in_is_tessellated_for_the_new_zoom() {
        let mut zoomed_out = frame(|f| {
            f.rotate(Radians(0.6));
            f.scale(0.25);
        });
        zoomed_out.fill_text(label());
        let meshes = cached();
        assert!(meshes > 0, "the first draw keeps its glyph meshes");

        assert_same_glyphs(|f| {
            f.rotate(Radians(0.6));
            f.scale(8.0);
        });
        assert!(cached() > meshes, "the new zoom reused the old meshes");
    }

    #[test]
    fn the_local_tolerance_keeps_device_error_within_a_tenth_of_a_pixel() {
        const PIXEL: f32 = 0.1;
        let slack = 1e-5;

        for k in -128..=128 {
            let s = (k as f32 / 16.0).exp2();

            for r in [0.0, 0.6, std::f32::consts::FRAC_PI_2, 2.5] {
                for (sx, sy) in [(1.0, 1.0), (1.0, 0.3), (0.3, 1.0), (-1.0, 1.0)] {
                    let transform =
                        Transform::scale(sx * s, sy * s).then_rotate(lyon::math::Angle::radians(r));
                    // Rotation and mirroring leave the stretches as they are.
                    let sigma_max = s * f32::max(sx.abs(), sy.abs());

                    let bucket = bucket(&transform).expect("a finite scale has a bucket");
                    let error = local_tolerance(bucket) * sigma_max;

                    assert!(
                        error <= PIXEL * (1.0 + slack),
                        "scale {s} rotation {r} ({sx}, {sy}): error {error} is too coarse"
                    );
                    assert!(
                        error >= PIXEL / 2f32.powf(0.25) * (1.0 - slack),
                        "scale {s} rotation {r} ({sx}, {sy}): error {error} is wastefully fine"
                    );
                }
            }
        }
    }

    #[test]
    fn the_glyph_mesh_cache_is_bounded() {
        let mut checked = false;

        label().visit_glyphs(|glyph| {
            let geometry::Glyph::Outline(outline) = glyph else {
                return;
            };
            if checked {
                return;
            }
            checked = true;

            MESHES.with_borrow_mut(|cache| {
                *cache = Cache::default();

                for i in 0..MAX_MESHES as i32 {
                    cache.insert((outline.key, i), Rc::new(LocalMesh::new()));
                }
            });

            // Bucket -1 is not among the filled ones, and its tolerance is sane.
            let _ = mesh(&outline, -1, &mut FillTessellator::new());

            assert_eq!(cached(), 1, "a full cache is emptied before it grows");
        });

        assert!(checked, "the label has an outline");
    }

    #[test]
    fn the_glyph_mesh_cache_is_bounded_by_bytes() {
        let mut checked = false;

        label().visit_glyphs(|glyph| {
            let geometry::Glyph::Outline(outline) = glyph else {
                return;
            };
            if checked {
                return;
            }
            checked = true;

            MESHES.with_borrow_mut(|cache| {
                *cache = Cache::default();

                // One huge mesh: far under the entry limit, one vertex under the byte one.
                let mut big = LocalMesh::new();
                big.vertices = vec![[0.0, 0.0]; MAX_MESH_BYTES / size_of::<[f32; 2]>() - 1];
                cache.insert((outline.key, 1000), Rc::new(big));
            });
            assert_eq!(cached(), 1);

            let _ = mesh(&outline, 0, &mut FillTessellator::new());

            assert_eq!(
                cached(),
                1,
                "a cache full of bytes is emptied before it grows"
            );
        });

        assert!(checked, "the label has an outline");
    }

    #[test]
    fn a_transform_stretched_past_any_real_zoom_gets_no_bucket() {
        assert_eq!(bucket(&Transform::scale(1e13, 1e13)), None);
    }

    #[test]
    fn a_collapsed_or_infinite_transform_gets_no_bucket() {
        assert_eq!(bucket(&Transform::scale(0.0, 0.0)), None);
        assert_eq!(bucket(&Transform::scale(f32::INFINITY, 1.0)), None);
        assert_eq!(bucket(&Transform::scale(f32::NAN, 1.0)), None);
    }
}
