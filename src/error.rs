use alloc::string::String;

/// The kind of error that occurred during path element normalization or validation.
///
/// See [`Error`] for the full error type, which also carries the original input string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The name is empty (or becomes empty after whitespace trimming).
    Empty,

    /// The name is `.`, the current directory marker.
    CurrentDirectoryMarker,

    /// The name is `..`, the parent directory marker.
    ParentDirectoryMarker,

    /// The name contains a forward slash (`/`), which is a path separator.
    ContainsForwardSlash,

    /// The name contains a null byte (`\0`), which all OSes treat as a string
    /// terminator, silently truncating the name.
    ContainsNullByte,

    /// The name contains a C0 control character (U+0001--U+001F).
    ContainsControlCharacter,

    /// The name contains a BOM (U+FEFF).
    ContainsBom,

    /// The byte input is not valid UTF-8.
    InvalidUtf8,

    /// The name contains a Unicode code point that is not assigned in the
    /// version of Unicode used by this crate.
    ContainsUnassignedChar,

    /// Apple's `CFStringGetFileSystemRepresentation` failed.
    /// This should never occur in practice, since validation runs before this.
    #[cfg(any(target_vendor = "apple", docsrs))]
    #[cfg_attr(docsrs, doc(cfg(target_vendor = "apple")))]
    GetFileSystemRepresentationError,
}

impl ErrorKind {
    /// Converts this error kind into an [`Error`], attaching the original input string.
    pub(crate) fn into_error(self, original: impl Into<String>) -> Error {
        Error {
            original: original.into(),
            kind: self,
        }
    }
}

impl core::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty => f.write_str("empty path element"),
            Self::CurrentDirectoryMarker => f.write_str("current directory marker"),
            Self::ParentDirectoryMarker => f.write_str("parent directory marker"),
            Self::ContainsForwardSlash => f.write_str("contains forward slash"),
            Self::ContainsNullByte => f.write_str("contains null byte"),
            Self::ContainsControlCharacter => f.write_str("contains control character"),
            Self::ContainsBom => f.write_str("contains BOM"),
            Self::InvalidUtf8 => f.write_str("invalid UTF-8"),
            Self::ContainsUnassignedChar => f.write_str("contains unassigned character"),
            #[cfg(any(target_vendor = "apple", docsrs))]
            Self::GetFileSystemRepresentationError => {
                f.write_str("CFStringGetFileSystemRepresentation failed")
            }
        }
    }
}

/// An error that occurred during path element normalization or validation.
///
/// Contains the [`ErrorKind`] and the original input string that caused the error.
///
/// ```
/// use normalized_path::{ErrorKind, PathElementCS};
///
/// assert!(PathElementCS::new("hello.txt").is_ok());
///
/// assert_eq!(PathElementCS::new("a/b").unwrap_err().original(), "a/b");
///
/// assert_eq!(PathElementCS::new("").unwrap_err().kind(), ErrorKind::Empty);
/// assert_eq!(PathElementCS::new(".").unwrap_err().kind(), ErrorKind::CurrentDirectoryMarker);
/// assert_eq!(PathElementCS::new("..").unwrap_err().kind(), ErrorKind::ParentDirectoryMarker);
/// assert_eq!(PathElementCS::new("a/b").unwrap_err().kind(), ErrorKind::ContainsForwardSlash);
/// assert_eq!(PathElementCS::new("a\0b").unwrap_err().kind(), ErrorKind::ContainsNullByte);
/// assert_eq!(PathElementCS::new("a\x01b").unwrap_err().kind(), ErrorKind::ContainsControlCharacter);
/// assert_eq!(PathElementCS::new("\u{FEFF}a").unwrap_err().kind(), ErrorKind::ContainsBom);
/// assert_eq!(PathElementCS::from_bytes(b"\xff").unwrap_err().kind(), ErrorKind::InvalidUtf8);
/// assert_eq!(PathElementCS::new("\u{0378}").unwrap_err().kind(), ErrorKind::ContainsUnassignedChar);
/// ```
#[derive(Debug)]
pub struct Error {
    original: String,
    kind: ErrorKind,
}

impl Error {
    /// Returns the kind of error.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the original input string that caused the error.
    #[must_use]
    pub fn original(&self) -> &str {
        &self.original
    }

    /// Consumes `self` and returns the original input string.
    #[must_use]
    pub fn into_original(self) -> String {
        self.original
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.original.is_empty() {
            write!(f, "{}", self.kind)
        } else {
            write!(f, "{}: {:?}", self.kind, self.original)
        }
    }
}

impl core::error::Error for Error {}

/// A [`Result`](core::result::Result) type alias using this crate's [`Error`].
pub type Result<T> = core::result::Result<T, Error>;

/// A [`Result`](core::result::Result) type alias using [`ErrorKind`] directly.
///
/// Used by internal normalization functions that do not have access to the
/// original input string. The [`PathElement`](crate::PathElementGeneric) constructors
/// convert `ResultKind` into [`Result`] by attaching the original.
pub type ResultKind<T> = core::result::Result<T, ErrorKind>;

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::{String, ToString};

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::ErrorKind;

    #[test]
    fn error_kind_display() {
        assert_eq!(ErrorKind::Empty.to_string(), "empty path element");
        assert_eq!(
            ErrorKind::CurrentDirectoryMarker.to_string(),
            "current directory marker"
        );
        assert_eq!(
            ErrorKind::ParentDirectoryMarker.to_string(),
            "parent directory marker"
        );
        assert_eq!(
            ErrorKind::ContainsForwardSlash.to_string(),
            "contains forward slash"
        );
        assert_eq!(
            ErrorKind::ContainsNullByte.to_string(),
            "contains null byte"
        );
        assert_eq!(
            ErrorKind::ContainsControlCharacter.to_string(),
            "contains control character"
        );
        assert_eq!(ErrorKind::ContainsBom.to_string(), "contains BOM");
        assert_eq!(ErrorKind::InvalidUtf8.to_string(), "invalid UTF-8");
        assert_eq!(
            ErrorKind::ContainsUnassignedChar.to_string(),
            "contains unassigned character"
        );
    }

    #[test]
    fn into_error_stores_original() {
        let err = ErrorKind::Empty.into_error(String::from("  "));
        assert_eq!(err.kind(), ErrorKind::Empty);
        assert_eq!(err.original(), "  ");
    }

    #[test]
    fn into_error_empty_original() {
        let err = ErrorKind::Empty.into_error(String::new());
        assert_eq!(err.kind(), ErrorKind::Empty);
        assert_eq!(err.original(), "");
    }

    #[test]
    fn kind_roundtrip() {
        let err = ErrorKind::ContainsNullByte.into_error(String::from("a\0b"));
        assert_eq!(err.kind(), ErrorKind::ContainsNullByte);
    }

    #[test]
    fn into_original() {
        let err = ErrorKind::Empty.into_error(String::from("  "));
        assert_eq!(err.into_original(), "  ");
    }

    #[test]
    fn error_display_with_original() {
        let err = ErrorKind::ContainsForwardSlash.into_error(String::from("a/b"));
        assert_eq!(format!("{err}"), "contains forward slash: \"a/b\"");
    }

    #[test]
    fn error_display_empty_original() {
        let err = ErrorKind::Empty.into_error(String::new());
        assert_eq!(format!("{err}"), "empty path element");
    }

    #[test]
    fn error_debug() {
        let err = ErrorKind::Empty.into_error(String::from("."));
        let debug = format!("{err:?}");
        assert!(debug.contains("Empty"));
        assert!(debug.contains('.'));
    }

    #[test]
    fn path_element_error_has_original() {
        let err = crate::PathElementCS::new("a/b").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ContainsForwardSlash);
        assert_eq!(err.original(), "a/b");
    }

    #[test]
    fn path_element_error_empty() {
        let err = crate::PathElementCS::new("").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Empty);
        assert_eq!(err.original(), "");
    }

    #[test]
    fn path_element_error_dot() {
        let err = crate::PathElementCI::new(".").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::CurrentDirectoryMarker);
        assert_eq!(err.original(), ".");
    }

    #[test]
    fn path_element_error_dotdot() {
        let err = crate::PathElementCS::new("..").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ParentDirectoryMarker);
        assert_eq!(err.original(), "..");
    }

    #[test]
    fn path_element_error_null_byte() {
        let err = crate::PathElementCS::new("a\0b").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ContainsNullByte);
        assert_eq!(err.original(), "a\0b");
    }

    #[test]
    fn path_element_error_control_character() {
        let err = crate::PathElementCS::new("a\x01b").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ContainsControlCharacter);
        assert_eq!(err.original(), "a\x01b");
    }

    #[test]
    fn path_element_error_bom() {
        let err = crate::PathElementCS::new("\u{FEFF}hello").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ContainsBom);
        assert_eq!(err.original(), "\u{FEFF}hello");
    }

    #[test]
    fn path_element_error_whitespace_trimmed_to_empty() {
        let err = crate::PathElementCS::new("   ").unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Empty);
        assert_eq!(err.original(), "   ");
    }
}
