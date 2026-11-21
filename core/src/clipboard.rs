//! Access the clipboard.
use std::path::PathBuf;
use std::sync::Arc;

/// A set of clipboard requests.
#[derive(Debug, Clone)]
pub struct Clipboard {
    /// The read requests the runtime must fulfill.
    pub reads: Vec<(ClipboardKind, Kind)>,
    /// The content that must be written to the clipboard by the runtime,
    /// if any.
    pub write: Option<(ClipboardKind, Content)>,
}

impl Clipboard {
    /// Creates a new empty set of [`Clipboard`] requests.
    pub fn new() -> Self {
        Self {
            reads: Vec::new(),
            write: None,
        }
    }

    /// Merges the current [`Clipboard`] requests with others.
    pub fn merge(&mut self, other: &mut Self) {
        self.reads.append(&mut other.reads);
        // The latest write wins, but a primary write never replaces a pending
        // standard one: that would silently lose an explicit copy.
        self.write = match (self.write.take(), other.write.take()) {
            (Some((ClipboardKind::Standard, content)), Some((ClipboardKind::Primary, _))) => {
                Some((ClipboardKind::Standard, content))
            }
            (current, new) => new.or(current),
        };
    }
}

/// The kind of [`Clipboard`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardKind {
    /// The standard clipboard.
    Standard,

    /// The primary clipboard (select to copy, middle-click to paste).
    ///
    /// Only present in X11 and Wayland; requests are ignored on every other
    /// platform.
    Primary,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

/// A clipboard event.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The clipboard was read.
    Read(Result<Arc<Content>, Error>),

    /// The clipboard was written.
    Written(Result<(), Error>),
}

/// Some clipboard content.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum Content {
    Text(String),
    Html(String),
    #[cfg(feature = "image")]
    Image(Image),
    Files(Vec<PathBuf>),
}

impl From<String> for Content {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

#[cfg(feature = "image")]
impl From<Image> for Content {
    fn from(image: Image) -> Self {
        Self::Image(image)
    }
}

impl From<Vec<PathBuf>> for Content {
    fn from(files: Vec<PathBuf>) -> Self {
        Self::Files(files)
    }
}

/// The kind of some clipboard [`Content`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum Kind {
    Text,
    Html,
    #[cfg(feature = "image")]
    Image,
    Files,
}

/// A clipboard image.
#[cfg(feature = "image")]
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    /// The pixels of the image in RGBA format.
    pub rgba: crate::Bytes,

    /// The physical [`Size`](crate::Size) of the image.
    pub size: crate::Size<u32>,
}

/// A clipboard error.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The clipboard in the current environment is either not present or could not be accessed.
    ClipboardUnavailable,

    /// The native clipboard is not accessible due to being held by another party.
    ClipboardOccupied,

    /// The clipboard contents were not available in the requested format.
    /// This could either be due to the clipboard being empty or the clipboard contents having
    /// an incompatible format to the requested one
    ContentNotAvailable,

    /// The image or the text that was about the be transferred to/from the clipboard could not be
    /// converted to the appropriate format.
    ConversionFailure,

    /// Any error that doesn't fit the other error types.
    Unknown {
        /// A description only meant to help the developer that should not be relied on as a
        /// means to identify an error case during runtime.
        description: Arc<String>,
    },
}

/// Returns true if the platform has a primary clipboard (X11 and Wayland, on
/// Linux and the BSDs).
///
/// Everywhere else, widgets must behave as if the primary clipboard did not
/// exist: no middle-click paste and no select-to-copy.
pub const fn primary_selection_supported() -> bool {
    cfg!(all(
        unix,
        not(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "android",
            target_os = "emscripten"
        ))
    ))
}
