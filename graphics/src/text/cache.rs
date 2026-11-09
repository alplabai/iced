//! Cache text.
use crate::core::{Font, Size};
use crate::text;

use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use std::collections::hash_map;
use std::hash::{Hash, Hasher};

/// A store of recently used sections of text.
#[derive(Debug, Default)]
pub struct Cache {
    entries: FxHashMap<KeyHash, Entry>,
    aliases: FxHashMap<KeyHash, KeyHash>,
    recently_used: FxHashSet<KeyHash>,
    version: text::Version,
}

impl Cache {
    /// Creates a new empty [`Cache`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the text [`Entry`] with the given [`KeyHash`].
    pub fn get(&self, key: &KeyHash) -> Option<&Entry> {
        self.entries.get(key)
    }

    /// Allocates a text [`Entry`] if it is not already present in the [`Cache`].
    pub fn allocate(
        &mut self,
        font_system: &mut cosmic_text::FontSystem,
        key: Key<'_>,
        version: text::Version,
    ) -> (KeyHash, &mut Entry) {
        // A font loaded since the text was shaped can change its shape, so
        // the cache starts over when the font system changes (upstream #3352).
        if version != self.version {
            self.entries.clear();
            self.aliases.clear();
            self.recently_used.clear();
            self.version = version;
        }

        let hash = key.hash(FxHasher::default());

        if let Some(hash) = self.aliases.get(&hash) {
            let _ = self.recently_used.insert(*hash);

            return (*hash, self.entries.get_mut(hash).unwrap());
        }

        if let hash_map::Entry::Vacant(entry) = self.entries.entry(hash) {
            let metrics =
                cosmic_text::Metrics::new(key.size, key.line_height.max(f32::MIN_POSITIVE));
            let mut buffer = cosmic_text::Buffer::new(font_system, metrics);

            let max_height = key.bounds.height.max(key.line_height);

            buffer.set_size(Some(key.bounds.width), Some(max_height));

            buffer.set_wrap(text::to_wrap(key.wrapping));
            buffer.set_ellipsize(text::to_ellipsize(key.ellipsis, max_height));

            buffer.set_text(
                key.content,
                &text::to_attributes(key.font),
                text::to_shaping(key.shaping, key.content, key.font),
                None,
            );
            buffer.shape_until_scroll(font_system, false);

            let bounds = text::align(&mut buffer, font_system, key.align_x);

            let _ = entry.insert(Entry {
                buffer,
                min_bounds: bounds,
            });

            for bounds in [
                bounds,
                Size {
                    width: key.bounds.width,
                    ..bounds
                },
            ] {
                if key.bounds != bounds {
                    let _ = self
                        .aliases
                        .insert(Key { bounds, ..key }.hash(FxHasher::default()), hash);
                }
            }
        }

        let _ = self.recently_used.insert(hash);

        (hash, self.entries.get_mut(&hash).unwrap())
    }

    /// Trims the [`Cache`].
    ///
    /// This will clear the sections of text that have not been used since the last `trim`.
    pub fn trim(&mut self) {
        self.entries
            .retain(|key, _| self.recently_used.contains(key));

        self.aliases
            .retain(|_, value| self.recently_used.contains(value));

        self.recently_used.clear();
    }
}

/// A cache key representing a section of text.
#[derive(Debug, Clone, Copy)]
pub struct Key<'a> {
    /// The content of the text.
    pub content: &'a str,
    /// The size of the text.
    pub size: f32,
    /// The line height of the text.
    pub line_height: f32,
    /// The [`Font`] of the text.
    pub font: Font,
    /// The bounds of the text.
    pub bounds: Size,
    /// The shaping strategy of the text.
    pub shaping: text::Shaping,
    /// The alignment of the text.
    pub align_x: text::Alignment,
    /// The wrapping strategy of the text.
    pub wrapping: text::Wrapping,
    /// The ellipsis strategy of the text.
    pub ellipsis: text::Ellipsis,
}

impl Key<'_> {
    fn hash<H: Hasher>(self, mut hasher: H) -> KeyHash {
        self.content.hash(&mut hasher);
        self.size.to_bits().hash(&mut hasher);
        self.line_height.to_bits().hash(&mut hasher);
        self.font.hash(&mut hasher);
        self.bounds.width.to_bits().hash(&mut hasher);
        self.bounds.height.to_bits().hash(&mut hasher);
        self.shaping.hash(&mut hasher);
        self.align_x.hash(&mut hasher);

        hasher.finish()
    }
}

/// The hash of a [`Key`].
pub type KeyHash = u64;

/// A cache entry.
#[derive(Debug)]
pub struct Entry {
    /// The buffer of text, ready for drawing.
    pub buffer: cosmic_text::Buffer,
    /// The minimum bounds of the text.
    pub min_bounds: Size,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::font;

    fn key(font: Font) -> Key<'static> {
        Key {
            content: "1.00",
            size: 16.0,
            line_height: 20.0,
            font,
            bounds: Size::new(100.0, 20.0),
            shaping: text::Shaping::Basic,
            align_x: text::Alignment::Default,
            wrapping: text::Wrapping::default(),
            ellipsis: text::Ellipsis::default(),
        }
    }

    #[test]
    fn fonts_differing_only_in_features_do_not_collide() {
        let plain = key(Font::DEFAULT).hash(FxHasher::default());
        let tabular = key(Font::DEFAULT.features(font::Features::TNUM)).hash(FxHasher::default());

        assert_ne!(plain, tabular);
    }

    /// Upstream #3352: text shaped with a fallback before its font was
    /// loaded stayed in the cache with the fallback's shape.
    #[test]
    fn reshapes_text_when_its_font_is_loaded() {
        use std::sync::Arc;

        // The icon font is there so the font system can always shape;
        // Fira Sans, the font the text asks for, is loaded only later.
        let mut db = cosmic_text::fontdb::Database::new();
        let _ = db.load_font_source(cosmic_text::fontdb::Source::Binary(Arc::new(
            include_bytes!("../../fonts/Iced-Icons.ttf").to_vec(),
        )));
        let mut font_system =
            cosmic_text::FontSystem::new_with_locale_and_db("en-US".to_owned(), db);
        let mut cache = Cache::new();
        let version = text::Version::default();
        let key = || Key {
            content: "Hello, world!",
            bounds: Size::new(1000.0, 1000.0),
            shaping: text::Shaping::Advanced,
            ..key(Font::new("Fira Sans"))
        };

        let unshaped = cache
            .allocate(&mut font_system, key(), version)
            .1
            .min_bounds;

        assert_eq!(
            cache
                .allocate(&mut font_system, key(), version)
                .1
                .min_bounds,
            unshaped,
            "text was reshaped without a font being loaded"
        );

        let _ = font_system
            .db_mut()
            .load_font_source(cosmic_text::fontdb::Source::Binary(Arc::new(
                include_bytes!("../../fonts/FiraSans-Regular.ttf").to_vec(),
            )));

        assert_ne!(
            cache
                .allocate(&mut font_system, key(), text::Version(version.0 + 1))
                .1
                .min_bounds,
            unshaped,
            "text was not reshaped after its font was loaded"
        );
    }
}
