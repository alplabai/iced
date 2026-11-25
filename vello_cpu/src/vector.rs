use crate::core::svg::{Data, Handle, Svg};
use crate::core::{Color, Rectangle, Size};

use resvg::tiny_skia;
use resvg::usvg;
use rustc_hash::{FxHashMap, FxHashSet};

use std::cell::RefCell;
use std::collections::hash_map;
use std::fs;
use std::panic;
use std::sync::Arc;

#[derive(Debug)]
pub struct Pipeline {
    cache: RefCell<Cache>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            cache: RefCell::new(Cache::default()),
        }
    }

    pub fn viewport_dimensions(&self, handle: &Handle) -> Size<u32> {
        self.cache
            .borrow_mut()
            .viewport_dimensions(handle)
            .unwrap_or(Size::new(0, 0))
    }

    pub fn draw(
        &mut self,
        svg: &Svg,
        bounds: Rectangle,
        renderer: &mut vello_cpu::RenderContext,
        scale_factor: f32,
    ) {
        let raster = self.cache.borrow_mut().draw(
            &svg.handle,
            svg.color,
            svg.current_color,
            Size::new(
                (bounds.width * scale_factor) as u32,
                (bounds.height * scale_factor) as u32,
            ),
        );

        let Some(raster) = raster else {
            return;
        };

        let width = f32::from(raster.width());
        let height = f32::from(raster.height());
        let width_scale = bounds.width / width;
        let height_scale = bounds.height / height;

        let transform = vello_cpu::kurbo::Affine::translate(vello_cpu::kurbo::Vec2::new(
            -f64::from(width) / 2.0,
            -f64::from(height) / 2.0,
        ))
        .then_rotate(f64::from(svg.rotation.0))
        .then_translate(vello_cpu::kurbo::Vec2::new(
            f64::from(width) / 2.0,
            f64::from(height) / 2.0,
        ))
        .then_scale_non_uniform(f64::from(width_scale), f64::from(height_scale))
        .then_translate(vello_cpu::kurbo::Vec2::new(
            f64::from(bounds.x),
            f64::from(bounds.y),
        ))
        .then_scale(f64::from(scale_factor));

        renderer.set_paint(vello_cpu::peniko::Brush::Image(
            vello_cpu::peniko::ImageBrush {
                image: vello_cpu::ImageSource::Pixmap(raster),
                sampler: vello_cpu::peniko::ImageSampler::new()
                    .with_quality(vello_cpu::peniko::ImageQuality::Low)
                    .with_alpha(svg.opacity),
            },
        ));

        renderer.set_transform(transform);

        renderer.fill_rect(&crate::into_rect(Rectangle {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }));

        renderer.reset_transform();
    }

    pub fn trim_cache(&mut self) {
        self.cache.borrow_mut().trim();
    }
}

#[derive(Default)]
struct Cache {
    trees: FxHashMap<Key, Option<resvg::usvg::Tree>>,
    tree_hits: FxHashSet<Key>,
    rasters: FxHashMap<RasterKey, Arc<vello_cpu::Pixmap>>,
    raster_hits: FxHashSet<RasterKey>,
    #[cfg(feature = "svg-text")]
    fontdb: Option<(crate::graphics::text::Version, Arc<usvg::fontdb::Database>)>,
}

/// Key for identifying SVGs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    id: u64,
    current_color: ColorKey,
}

impl Key {
    fn new(id: u64, color: Option<Color>) -> Self {
        Self {
            id,
            current_color: color.map(Color::into_rgba8),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RasterKey {
    id: u64,
    color: ColorKey,
    current_color: ColorKey,
    size: Size<u32>,
}

type ColorKey = Option<[u8; 4]>;

impl Cache {
    fn load(&mut self, handle: &Handle, current_color: Option<Color>) -> Option<&usvg::Tree> {
        let id = handle.id();

        // Reuse the `cosmic-text` font database instead of scanning the system
        // fonts again, and clone it again once a font is loaded (pop-os/iced 5013f63).
        #[cfg(feature = "svg-text")]
        {
            let font_system = crate::graphics::text::font_system()
                .read()
                .expect("Read font system");
            let version = font_system.version();

            if self
                .fontdb
                .as_ref()
                .is_none_or(|(cached, _)| *cached != version)
            {
                self.fontdb = Some((version, Arc::new(font_system.db().clone())));
            }
        }

        let options = usvg::Options {
            #[cfg(feature = "svg-text")]
            fontdb: self
                .fontdb
                .as_ref()
                .expect("fontdb must be initialized")
                .1
                .clone(),
            style_sheet: current_color.map(crate::core::svg::color_style_sheet),
            ..usvg::Options::default()
        };

        let key = Key::new(id, current_color);
        if let hash_map::Entry::Vacant(entry) = self.trees.entry(key) {
            let svg = match handle.data() {
                Data::Path(path) => fs::read_to_string(path)
                    .ok()
                    .and_then(|contents| usvg::Tree::from_str(&contents, &options).ok()),
                Data::Bytes(bytes) => usvg::Tree::from_data(bytes, &options).ok(),
            };

            let _ = entry.insert(svg);
        }

        let _ = self.tree_hits.insert(key);
        self.trees.get(&key).unwrap().as_ref()
    }

    fn viewport_dimensions(&mut self, handle: &Handle) -> Option<Size<u32>> {
        let tree = self.load(handle, None)?;
        let size = tree.size();

        Some(Size::new(size.width() as u32, size.height() as u32))
    }

    fn draw(
        &mut self,
        handle: &Handle,
        color: Option<Color>,
        current_color: Option<Color>,
        size: Size<u32>,
    ) -> Option<Arc<vello_cpu::Pixmap>> {
        if size.width == 0 || size.height == 0 {
            return None;
        }

        let key = RasterKey {
            id: handle.id(),
            color: color.map(Color::into_rgba8),
            current_color: current_color.map(Color::into_rgba8),
            size,
        };

        #[allow(clippy::map_entry)]
        if !self.rasters.contains_key(&key) {
            let tree = self.load(handle, current_color)?;

            let mut image = vello_cpu::Pixmap::new(size.width as u16, size.height as u16);

            let tree_size = tree.size().to_int_size();

            let target_size = if size.width > size.height {
                tree_size.scale_to_width(size.width)
            } else {
                tree_size.scale_to_height(size.height)
            };

            let transform = if let Some(target_size) = target_size {
                let tree_size = tree_size.to_size();
                let target_size = target_size.to_size();

                tiny_skia::Transform::from_scale(
                    target_size.width() / tree_size.width(),
                    target_size.height() / tree_size.height(),
                )
            } else {
                tiny_skia::Transform::default()
            };

            // SVG rendering can panic on malformed or complex vectors.
            // We catch panics to prevent crashes and continue gracefully.
            let render = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                resvg::render(
                    tree,
                    transform,
                    &mut tiny_skia::PixmapMut::from_bytes(
                        image.data_as_u8_slice_mut(),
                        size.width,
                        size.height,
                    )
                    .expect("Build mutable pixmap"),
                );
            }));

            if let Err(error) = render {
                log::warn!("SVG rendering for {handle:?} panicked: {error:?}");
            }

            if let Some([r, g, b, mask_alpha]) = key.color {
                // Apply color filter
                for pixel in image.data_mut() {
                    let alpha = scale_coverage(pixel.a, mask_alpha);

                    *pixel =
                        crate::into_color(Color::from_rgba8(r, g, b, f32::from(alpha) / 255.0))
                            .premultiply()
                            .to_rgba8();
                }
            } else {
                // Swap R and B channels for `softbuffer` presentation
                for vello_cpu::color::PremulRgba8 { r, b, .. } in image.data_mut() {
                    std::mem::swap(r, b);
                }
            }

            let _ = self.rasters.insert(key, Arc::new(image));
        }

        let _ = self.raster_hits.insert(key);
        self.rasters.get(&key).map(Arc::clone)
    }

    fn trim(&mut self) {
        self.trees.retain(|key, _| self.tree_hits.contains(key));
        self.rasters.retain(|key, _| self.raster_hits.contains(key));

        self.tree_hits.clear();
        self.raster_hits.clear();
    }
}

/// Scales the coverage of a pixel by the alpha of a color mask, so that
/// anti-aliased edges survive and a translucent mask stays translucent. Same
/// rule as the wgpu renderer; a fork deviation from upstream #3542, which
/// replaces the coverage.
fn scale_coverage(coverage: u8, mask_alpha: u8) -> u8 {
    ((u16::from(coverage) * u16::from(mask_alpha) + 127) / 255) as u8
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("tree_hits", &self.tree_hits)
            .field("rasters", &self.rasters)
            .field("raster_hits", &self.raster_hits)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_alpha_scales_coverage() {
        assert_eq!(scale_coverage(77, 255), 77);
        assert_eq!(scale_coverage(255, 128), 128);
        assert_eq!(scale_coverage(100, 128), 50);
        assert_eq!(scale_coverage(0, 128), 0);
    }

    const SQUARE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"><rect width="4" height="4" fill="currentColor"/></svg>"#;

    /// Draws `SQUARE` and returns its centre pixel as premultiplied RGBA.
    fn centre(cache: &mut Cache, handle: &Handle, current_color: Option<Color>) -> [u8; 4] {
        let pixmap = cache
            .draw(handle, None, current_color, Size::new(4, 4))
            .expect("SVG should be drawn");
        let pixel = pixmap.data()[2 * 4 + 2];

        // `draw` swaps red and blue for `softbuffer`.
        [pixel.b, pixel.g, pixel.r, pixel.a]
    }

    #[test]
    fn current_color_fills_svg() {
        let handle = Handle::from_memory(SQUARE);
        let mut cache = Cache::default();

        assert_eq!(centre(&mut cache, &handle, None), [0, 0, 0, 255]);
        assert_eq!(
            centre(&mut cache, &handle, Some(Color::from_rgb8(255, 0, 0))),
            [255, 0, 0, 255]
        );
        assert_eq!(
            centre(&mut cache, &handle, Some(Color::from_rgb8(0, 0, 255))),
            [0, 0, 255, 255]
        );
        assert_eq!(centre(&mut cache, &handle, None), [0, 0, 0, 255]);
    }

    #[test]
    fn current_color_keeps_its_alpha() {
        let handle = Handle::from_memory(SQUARE);
        let mut cache = Cache::default();

        let [red, green, blue, alpha] =
            centre(&mut cache, &handle, Some(Color::from_rgba8(255, 0, 0, 0.5)));

        assert!((120..=136).contains(&alpha), "alpha was {alpha}");
        assert_eq!([red, green, blue], [alpha, 0, 0]);
    }

    #[cfg(feature = "svg-text")]
    #[test]
    fn font_loaded_later_reaches_the_next_svg() {
        let mut cache = Cache::default();
        let _ = cache.load(&Handle::from_memory(SQUARE), None);
        let (version, before) = cache.fontdb.clone().expect("fontdb after a load");

        // The SVG database is the font system's, not a second system scan
        // (unless another test loaded a font in between).
        {
            let font_system = crate::graphics::text::font_system()
                .read()
                .expect("Read font system");

            if font_system.version() == version {
                assert_eq!(before.len(), font_system.db().len());
            }
        }

        // Owned bytes, so the load is not skipped as a duplicate of another test's.
        crate::graphics::text::font_system()
            .write()
            .expect("Write font system")
            .load_font(std::borrow::Cow::Owned(
                include_bytes!("../../graphics/fonts/Iced-Icons.ttf").to_vec(),
            ));

        let other: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"/>"#;
        let _ = cache.load(&Handle::from_memory(other), None);
        let (new_version, after) = cache.fontdb.clone().expect("fontdb after a load");

        assert_ne!(new_version, version);
        assert!(after.len() > before.len());
    }
}
