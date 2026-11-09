use crate::core;
use crate::core::alignment;
use crate::core::text::{Alignment, Ellipsis, LineHeight, Paragraph, Shaping, Wrapping};
use crate::core::{Color, Font, Pixels, Point, Size, Vector};
use crate::geometry::Path;
use crate::text;

/// A bunch of text that can be drawn to a canvas
#[derive(Debug, Clone)]
pub struct Text {
    /// The contents of the text
    pub content: String,
    /// The position of the text relative to the alignment properties.
    ///
    /// By default, this position will be relative to the top-left corner coordinate meaning that
    /// if the horizontal and vertical alignments are unchanged, this property will tell where the
    /// top-left corner of the text should be placed.
    ///
    /// By changing the horizontal_alignment and vertical_alignment properties, you are are able to
    /// change what part of text is placed at this positions.
    ///
    /// For example, when the horizontal_alignment and vertical_alignment are set to Center, the
    /// center of the text will be placed at the given position NOT the top-left coordinate.
    pub position: Point,
    /// The maximum horizontal space available for this [`Text`].
    ///
    /// Text will break into new lines when the width is reached.
    pub max_width: f32,
    /// The color of the text
    pub color: Color,
    /// The size of the text
    pub size: Pixels,
    /// The line height of the text.
    pub line_height: LineHeight,
    /// The font of the text
    pub font: Font,
    /// The horizontal alignment of the text
    pub align_x: Alignment,
    /// The vertical alignment of the text
    pub align_y: alignment::Vertical,
    /// The shaping strategy of the text.
    pub shaping: Shaping,
    /// The wrapping strategy of the text.
    pub wrapping: Wrapping,
    /// The ellipsis strategy of the text.
    pub ellipsis: Ellipsis,
}

/// The largest size, in logical pixels after the frame transform, that
/// `fill_text` draws from the glyph atlas (wgpu) or the glyph cache
/// (tiny_skia); bigger text is drawn as vector outlines
/// ([`Text::visit_glyphs`]), filled by the geometry backend, and stays sharp at
/// any size.
///
/// wgpu: one glyph bitmap this size takes a large share of an atlas texture.
/// The value assumes a desktop device whose `max_texture_dimension_2d` is at
/// least 4096 at a display scale factor of 2 or less (the frame does not know
/// the scale factor, so this compares logical pixels); a WebGL2 or downlevel
/// device (2048) needs a lower value.
///
/// tiny_skia has no atlas: the reason there is the memory of the CPU glyph
/// cache, which holds a bitmap per glyph per size.
///
/// The outlines are lyon meshes. Like every canvas mesh they are smoothed only
/// when the application enables multisampling (`antialiasing(true)`); atlas
/// glyphs carry their own coverage, so without it text edges change at this
/// size.
pub const MAX_ATLAS_TEXT_SIZE: f32 = 1024.0;

/// Largest glyph size, in pixels, whose outline-less bitmap (colour emoji,
/// sbix or CBDT strikes) is still drawn pixel by pixel; the fallback is one
/// path per pixel, so it costs size squared.
// shortcut: bigger bitmap-only glyphs are not drawn, because keeping them on
// the atlas needs the text routed per glyph; revisit with per-glyph routing.
const MAX_BITMAP_FALLBACK_SIZE: f32 = 128.0;

/// The most entries the outline map of the shared swash cache may hold before
/// it is emptied; it would otherwise grow with every glyph and size ever drawn
/// as outlines.
const MAX_SWASH_CACHE_ENTRIES: usize = 16 * 1024;

/// The most entries the image map of the shared swash cache may hold before it
/// is emptied. Only outline-less (bitmap) glyphs reach it, and each entry can
/// take up to `MAX_BITMAP_FALLBACK_SIZE`² × 4 bytes, so it gets a much lower
/// limit than the outline map.
const MAX_SWASH_IMAGE_ENTRIES: usize = 256;

thread_local! {
    static SWASH_CACHE: std::cell::Cell<Option<cosmic_text::SwashCache>> =
        const { std::cell::Cell::new(None) };
}

/// A per-thread [`cosmic_text::SwashCache`] that [`Text::draw_with`] reuses,
/// so glyph outlines are scaled once instead of on every call.
///
/// The cache is moved out of its slot while in use and put back on drop.
struct SharedSwashCache(Option<cosmic_text::SwashCache>);

impl SharedSwashCache {
    fn take() -> Self {
        let mut cache = SWASH_CACHE
            .with(std::cell::Cell::take)
            .unwrap_or_else(cosmic_text::SwashCache::new);

        // Replace the maps rather than `clear` them, so the memory of their
        // hash tables is released too.
        if cache.outline_command_cache.len() > MAX_SWASH_CACHE_ENTRIES {
            cache.outline_command_cache = std::collections::HashMap::default();
        }

        if cache.image_cache.len() > MAX_SWASH_IMAGE_ENTRIES {
            cache.image_cache = std::collections::HashMap::default();
        }

        Self(Some(cache))
    }
}

impl std::ops::Deref for SharedSwashCache {
    type Target = cosmic_text::SwashCache;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("Swash cache is present")
    }
}

impl std::ops::DerefMut for SharedSwashCache {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("Swash cache is present")
    }
}

impl Drop for SharedSwashCache {
    fn drop(&mut self) {
        let cache = self.0.take();
        let _ = SWASH_CACHE.try_with(|slot| slot.set(cache));
    }
}

fn bitmap_fallback_allowed(glyph_size: f32) -> bool {
    glyph_size <= MAX_BITMAP_FALLBACK_SIZE
}

impl Text {
    /// Whether this text, drawn through a transform with the positive uniform
    /// scale `scale`, is small enough for the glyph atlas (see
    /// [`MAX_ATLAS_TEXT_SIZE`]). A size or scale that is NaN or infinite does
    /// not fit, so it takes the vector path rather than the atlas.
    pub fn fits_atlas(&self, scale: f32) -> bool {
        self.size.0 * scale <= MAX_ATLAS_TEXT_SIZE
    }

    /// The laid-out paragraph, as the atlas path lays it out: same wrapping,
    /// ellipsis and bounds, so text does not reflow where the drawing changes.
    fn paragraph(&self) -> text::Paragraph {
        text::Paragraph::with_text(core::text::Text {
            content: &self.content,
            bounds: Size::new(self.max_width, f32::INFINITY),
            size: self.size,
            line_height: self.line_height,
            font: self.font,
            align_x: self.align_x,
            align_y: self.align_y,
            shaping: self.shaping,
            wrapping: self.wrapping,
            ellipsis: self.ellipsis,
            hint_factor: None,
        })
    }

    /// Computes the [`Path`]s of the [`Text`] and draws them using
    /// the given closure.
    pub fn draw_with(&self, mut f: impl FnMut(Path, Color)) {
        self.visit_glyphs(|glyph| match glyph {
            Glyph::Outline(outline) => f(outline.path(), outline.color),
            Glyph::Pixel(path, color) => f(path, color),
        });
    }

    /// Lays the [`Text`] out and hands every glyph to the given closure, in
    /// draw order: an outline with its cache key and position, or the pixels
    /// of an outline-less (bitmap) glyph.
    ///
    /// A backend can keep what it builds from an outline (a mesh, say) by
    /// [`GlyphKey`], since one key always has the same outline.
    ///
    /// The closure runs while the global font system's write lock is held, so
    /// it must not lay out text itself.
    pub fn visit_glyphs(&self, mut f: impl FnMut(Glyph<'_>)) {
        let paragraph = self.paragraph();

        let translation_x = match self.align_x {
            Alignment::Default | Alignment::Left | Alignment::Justified => self.position.x,
            Alignment::Center => self.position.x - paragraph.min_width() / 2.0,
            Alignment::Right => self.position.x - paragraph.min_width(),
        };

        let translation_y = {
            match self.align_y {
                alignment::Vertical::Top => self.position.y,
                alignment::Vertical::Center => self.position.y - paragraph.min_height() / 2.0,
                alignment::Vertical::Bottom => self.position.y - paragraph.min_height(),
            }
        };

        let buffer = paragraph.buffer();
        let mut swash_cache = SharedSwashCache::take();

        let mut font_system = text::font_system().write().expect("Write font system");

        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical_glyph = glyph.physical((0.0, 0.0), 1.0);

                let start_x = translation_x + glyph.x + glyph.x_offset;
                let start_y = translation_y + glyph.y_offset + run.line_y;
                let offset = Vector::new(start_x, start_y);

                // Outlines do not depend on the subpixel position (it is
                // applied through `offset`), so they are cached without it.
                // The raster fallback below needs the bins and keeps them.
                let mut outline_key = physical_glyph.cache_key;
                outline_key.x_bin = cosmic_text::SubpixelBin::Zero;
                outline_key.y_bin = cosmic_text::SubpixelBin::Zero;

                if let Some(commands) =
                    swash_cache.get_outline_commands(font_system.raw(), outline_key)
                {
                    f(Glyph::Outline(GlyphOutline {
                        key: GlyphKey(outline_key),
                        offset,
                        color: self.color,
                        commands,
                    }));
                } else {
                    if !bitmap_fallback_allowed(f32::from_bits(
                        physical_glyph.cache_key.font_size_bits,
                    )) {
                        continue;
                    }
                    // TODO: Raster image support for `Canvas`
                    let [r, g, b, a] = self.color.into_rgba8();

                    swash_cache.with_pixels(
                        font_system.raw(),
                        physical_glyph.cache_key,
                        cosmic_text::Color::rgba(r, g, b, a),
                        |x, y, color| {
                            f(Glyph::Pixel(
                                Path::rectangle(
                                    Point::new(x as f32, y as f32) + offset,
                                    Size::new(1.0, 1.0),
                                ),
                                Color::from_rgba8(
                                    color.r(),
                                    color.g(),
                                    color.b(),
                                    color.a() as f32 / 255.0,
                                ),
                            ));
                        },
                    );
                }
            }
        }
    }
}

/// A glyph of a [`Text`], as [`Text::visit_glyphs`] hands it out.
#[derive(Debug)]
pub enum Glyph<'a> {
    /// A glyph with a vector outline.
    Outline(GlyphOutline<'a>),
    /// One pixel of an outline-less (bitmap) glyph, as a unit square.
    Pixel(Path, Color),
}

/// Identifies a glyph outline: the key covers font, glyph, size, weight and
/// synthesis/hinting flags. Two glyphs with one key have the same outline in
/// glyph-local coordinates. It is valid only within the process-global font
/// system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey(cosmic_text::CacheKey);

/// The vector outline of one glyph of a [`Text`].
#[derive(Debug)]
pub struct GlyphOutline<'a> {
    /// The key of the outline.
    pub key: GlyphKey,
    /// Where the glyph-local origin (the pen position) lands in the frame.
    pub offset: Vector,
    /// The color of the glyph.
    pub color: Color,
    commands: &'a [cosmic_text::Command],
}

impl GlyphOutline<'_> {
    /// The outline placed at its position in the frame.
    pub fn path(&self) -> Path {
        self.path_at(self.offset)
    }

    /// The outline in glyph-local coordinates, y down, origin at the pen
    /// position; the same for every glyph with this key.
    pub fn local_path(&self) -> Path {
        self.path_at(Vector::ZERO)
    }

    fn path_at(&self, offset: Vector) -> Path {
        Path::new(|path| {
            use cosmic_text::Command;

            for command in self.commands {
                match command {
                    Command::MoveTo(p) => {
                        path.move_to(Point::new(p.x, -p.y) + offset);
                    }
                    Command::LineTo(p) => {
                        path.line_to(Point::new(p.x, -p.y) + offset);
                    }
                    Command::CurveTo(control_a, control_b, to) => {
                        path.bezier_curve_to(
                            Point::new(control_a.x, -control_a.y) + offset,
                            Point::new(control_b.x, -control_b.y) + offset,
                            Point::new(to.x, -to.y) + offset,
                        );
                    }
                    Command::QuadTo(control, to) => {
                        path.quadratic_curve_to(
                            Point::new(control.x, -control.y) + offset,
                            Point::new(to.x, -to.y) + offset,
                        );
                    }
                    Command::Close => {
                        path.close();
                    }
                }
            }
        })
    }
}

impl Default for Text {
    fn default() -> Text {
        Text {
            content: String::new(),
            position: Point::ORIGIN,
            max_width: f32::INFINITY,
            color: Color::BLACK,
            size: Pixels(16.0),
            line_height: LineHeight::Relative(1.2),
            font: Font::default(),
            align_x: Alignment::Default,
            align_y: alignment::Vertical::Top,
            shaping: Shaping::default(),
            wrapping: Wrapping::default(),
            ellipsis: Ellipsis::default(),
        }
    }
}

impl From<String> for Text {
    fn from(content: String) -> Text {
        Text {
            content,
            ..Default::default()
        }
    }
}

impl From<&str> for Text {
    fn from(content: &str) -> Text {
        String::from(content).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(size: f32) -> Text {
        Text {
            size: Pixels(size),
            ..Text::default()
        }
    }

    #[test]
    fn ordinary_sizes_use_the_atlas_and_huge_ones_do_not() {
        assert!(text(16.0).fits_atlas(1.0));
        assert!(text(10.0).fits_atlas(100.0));
        assert!(text(MAX_ATLAS_TEXT_SIZE).fits_atlas(1.0));
        assert!(!text(16.0).fits_atlas(200.0));
        assert!(!text(5000.0).fits_atlas(1.0));
    }

    #[test]
    fn a_size_that_is_not_a_number_is_not_sent_to_the_atlas() {
        assert!(!text(f32::NAN).fits_atlas(1.0));
        assert!(!text(f32::INFINITY).fits_atlas(1.0));
        assert!(!text(16.0).fits_atlas(f32::NAN));
    }

    /// `draw_with` lays text out with the text's own wrapping, so a text that
    /// does not wrap is one line below and above the atlas threshold alike.
    #[test]
    fn the_vector_path_keeps_the_wrapping_of_the_atlas_path() {
        let lines = |size: f32| {
            let t = Text {
                content: "alpha beta gamma delta epsilon zeta eta theta".into(),
                max_width: 80.0,
                wrapping: Wrapping::None,
                ..text(size)
            };
            t.paragraph().buffer().layout_runs().count()
        };
        let (below, above) = (
            lines(MAX_ATLAS_TEXT_SIZE - 1.0),
            lines(MAX_ATLAS_TEXT_SIZE + 1.0),
        );
        assert_eq!(below, above);
        assert!(below <= 1);
    }

    /// A bitmap-only glyph (colour emoji) costs one path per pixel, so past a
    /// small size it is skipped instead of freezing the frame.
    #[test]
    fn a_big_bitmap_only_glyph_is_not_drawn_pixel_by_pixel() {
        assert!(bitmap_fallback_allowed(16.0));
        assert!(bitmap_fallback_allowed(MAX_BITMAP_FALLBACK_SIZE));
        assert!(!bitmap_fallback_allowed(1500.0));
        assert!(!bitmap_fallback_allowed(f32::NAN));
    }

    fn outline_entries() -> usize {
        SharedSwashCache::take().outline_command_cache.len()
    }

    /// Drawing the same text again reuses the glyph outlines of the first
    /// draw instead of building a new cache.
    #[test]
    fn drawing_text_again_reuses_the_swash_cache() {
        let label = Text {
            content: "Rotated label 0123".into(),
            ..text(14.0)
        };

        let mut paths = 0;
        label.draw_with(|_, _| paths += 1);
        assert!(paths > 0);

        let after_first = outline_entries();
        assert!(after_first > 0, "outlines are kept after the first draw");

        label.draw_with(|_, _| ());
        assert_eq!(outline_entries(), after_first);
    }

    /// The same glyph at different subpixel positions shares one outline.
    ///
    /// The bins come from the glyph's position inside the layout, so a label
    /// repeating one glyph at fractional advances would otherwise store it once
    /// per bin.
    #[test]
    fn outlines_are_shared_across_subpixel_positions() {
        let label = Text {
            content: "lllllllllllllllllllllllllllllllllllllllllll".into(),
            ..text(13.3)
        };

        label.draw_with(|_, _| ());

        assert_eq!(outline_entries(), 1);
    }

    /// The shared cache is emptied once it passes its entry limit.
    #[test]
    fn the_swash_cache_is_bounded() {
        let mut cache = SharedSwashCache::take();
        let key = cosmic_text::CacheKey::new(
            cosmic_text::fontdb::ID::dummy(),
            0,
            16.0,
            (0.0, 0.0),
            cosmic_text::Weight::NORMAL,
            cosmic_text::CacheKeyFlags::empty(),
        )
        .0;

        for i in 0..=MAX_SWASH_CACHE_ENTRIES {
            let mut key = key;
            key.glyph_id = i as u16;
            key.font_size_bits = i as u32;
            let _ = cache.outline_command_cache.insert(key, None);
        }
        drop(cache);

        assert_eq!(outline_entries(), 0);
    }

    /// The image map has its own, much smaller limit.
    #[test]
    fn the_swash_image_cache_is_bounded() {
        let mut cache = SharedSwashCache::take();
        let key = cosmic_text::CacheKey::new(
            cosmic_text::fontdb::ID::dummy(),
            0,
            16.0,
            (0.0, 0.0),
            cosmic_text::Weight::NORMAL,
            cosmic_text::CacheKeyFlags::empty(),
        )
        .0;

        for i in 0..=MAX_SWASH_IMAGE_ENTRIES {
            let mut key = key;
            key.glyph_id = i as u16;
            let _ = cache.image_cache.insert(key, None);
        }
        drop(cache);

        assert_eq!(SharedSwashCache::take().image_cache.len(), 0);
    }
}
