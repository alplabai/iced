//! Load and use fonts.
use bitflags::bitflags;
use std::hash::Hash;

/// A font.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Font {
    /// The [`Family`] of the [`Font`].
    pub family: Family,
    /// The [`Weight`] of the [`Font`].
    pub weight: Weight,
    /// The [`Stretch`] of the [`Font`].
    pub stretch: Stretch,
    /// The [`Style`] of the [`Font`].
    pub style: Style,
    /// The OpenType [`Features`] of the [`Font`].
    pub features: Features,
}

impl Font {
    /// A non-monospaced sans-serif font with normal [`Weight`].
    pub const DEFAULT: Font = Font {
        family: Family::SansSerif,
        weight: Weight::Normal,
        stretch: Stretch::Normal,
        style: Style::Normal,
        features: Features::empty(),
    };

    /// A monospaced font with normal [`Weight`].
    pub const MONOSPACE: Font = Font {
        family: Family::Monospace,
        ..Self::DEFAULT
    };

    /// Creates a [`Font`] with the given [`Family::Name`] and default attributes.
    pub const fn new(name: &'static str) -> Self {
        Self {
            family: Family::Name(name),
            ..Self::DEFAULT
        }
    }

    /// Creates a [`Font`] with the given [`Family`] and default attributes.
    pub fn with_family(family: impl Into<Family>) -> Self {
        Font {
            family: family.into(),
            ..Self::DEFAULT
        }
    }

    /// Sets the [`Weight`] of the [`Font`].
    pub const fn weight(self, weight: Weight) -> Self {
        Self { weight, ..self }
    }

    /// Sets the [`Stretch`] of the [`Font`].
    pub const fn stretch(self, stretch: Stretch) -> Self {
        Self { stretch, ..self }
    }

    /// Sets the [`Style`] of the [`Font`].
    pub const fn style(self, style: Style) -> Self {
        Self { style, ..self }
    }

    /// Sets the OpenType [`Features`] of the [`Font`].
    ///
    /// A feature only has an effect if the font face supports it.
    pub const fn features(self, features: Features) -> Self {
        Self { features, ..self }
    }
}

bitflags! {
    /// A set of OpenType features to toggle when shaping text.
    ///
    /// A feature only has an effect if the font face supports it, and only
    /// when text is shaped with [`Shaping::Advanced`](crate::text::Shaping::Advanced).
    /// [`Shaping::Auto`](crate::text::Shaping::Auto) is promoted to advanced
    /// shaping for a font that requests features, even for ASCII text; an
    /// explicit [`Shaping::Basic`](crate::text::Shaping::Basic) ignores them.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Features: u8 {
        /// Tabular figures (`tnum`): all digits have the same advance.
        const TNUM = 1 << 0;
        /// Slashed zero (`zero`).
        const ZERO = 1 << 1;
        /// Disables standard and contextual ligatures (`liga` and `clig`).
        const LIGA_OFF = 1 << 2;
        /// Disables contextual alternates (`calt`).
        const CALT_OFF = 1 << 3;
        /// Small capitals (`smcp`).
        const SMCP = 1 << 4;
        /// Stylistic set 1 (`ss01`).
        const SS01 = 1 << 5;
    }
}

impl From<&'static str> for Font {
    fn from(name: &'static str) -> Self {
        Font::new(name)
    }
}

impl From<Family> for Font {
    fn from(family: Family) -> Self {
        Font::with_family(family)
    }
}

/// A font family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Family {
    /// The name of a font family of choice.
    Name(&'static str),

    /// Serif fonts represent the formal text style for a script.
    Serif,

    /// Glyphs in sans-serif fonts, as the term is used in CSS, are generally low
    /// contrast and have stroke endings that are plain — without any flaring,
    /// cross stroke, or other ornamentation.
    #[default]
    SansSerif,

    /// Glyphs in cursive fonts generally use a more informal script style, and
    /// the result looks more like handwritten pen or brush writing than printed
    /// letterwork.
    Cursive,

    /// Fantasy fonts are primarily decorative or expressive fonts that contain
    /// decorative or expressive representations of characters.
    Fantasy,

    /// The sole criterion of a monospace font is that all glyphs have the same
    /// fixed width.
    Monospace,
}

impl Family {
    /// A list of all the different standalone family variants.
    pub const VARIANTS: &[Self] = &[
        Self::Serif,
        Self::SansSerif,
        Self::Cursive,
        Self::Fantasy,
        Self::Monospace,
    ];

    /// Creates a [`Family::Name`] from the given string.
    ///
    /// The name is interned in a global cache and never freed.
    pub fn name(name: &str) -> Self {
        use rustc_hash::FxHashSet;
        use std::sync::{LazyLock, Mutex};

        static NAMES: LazyLock<Mutex<FxHashSet<&'static str>>> = LazyLock::new(Mutex::default);

        let mut names = NAMES.lock().expect("lock font name cache");

        let Some(name) = names.get(name) else {
            let name: &'static str = name.to_owned().leak();
            let _ = names.insert(name);

            return Self::Name(name);
        };

        Self::Name(name)
    }
}

impl From<&str> for Family {
    fn from(name: &str) -> Self {
        Family::name(name)
    }
}

impl std::fmt::Display for Family {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Family::Name(name) => name,
            Family::Serif => "Serif",
            Family::SansSerif => "Sans-serif",
            Family::Cursive => "Cursive",
            Family::Fantasy => "Fantasy",
            Family::Monospace => "Monospace",
        })
    }
}

/// The weight of some text.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Weight {
    Thin,
    ExtraLight,
    Light,
    #[default]
    Normal,
    Medium,
    Semibold,
    Bold,
    ExtraBold,
    Black,
}

/// The width of some text.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Stretch {
    UltraCondensed,
    ExtraCondensed,
    Condensed,
    SemiCondensed,
    #[default]
    Normal,
    SemiExpanded,
    Expanded,
    ExtraExpanded,
    UltraExpanded,
}

/// The style of some text.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Style {
    #[default]
    Normal,
    Italic,
    Oblique,
}

/// A font error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    fn hash(font: Font) -> u64 {
        let mut hasher = DefaultHasher::new();
        font.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn features_default_to_empty() {
        assert_eq!(Font::DEFAULT.features, Features::empty());
        assert_eq!(Font::default(), Font::DEFAULT);
    }

    #[test]
    fn features_builder_sets_features() {
        let font = Font::MONOSPACE.features(Features::TNUM | Features::ZERO);

        assert_eq!(font.family, Family::Monospace);
        assert_eq!(font.features, Features::TNUM | Features::ZERO);
    }

    #[test]
    fn fonts_differing_only_in_features_are_distinct() {
        let plain = Font::DEFAULT;
        let tabular = Font::DEFAULT.features(Features::TNUM);

        assert_ne!(plain, tabular);
        assert_ne!(hash(plain), hash(tabular));
    }
}
