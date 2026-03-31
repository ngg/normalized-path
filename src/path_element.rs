use alloc::borrow::Cow;
use alloc::string::String;
#[cfg(feature = "std")]
use std::ffi::{OsStr, OsString};

use crate::Result;
use crate::case_sensitivity::{CaseInsensitive, CaseSensitive, CaseSensitivity};
use crate::normalize::{normalize_ci_from_normalized_cs, normalize_cs};
use crate::os::os_compatible_from_normalized_cs;
use crate::utils::SubstringOrOwned;

/// Non-generic core of `with_case_sensitivity`: validates, normalizes, and computes
/// the OS-compatible form, returning the two `SubstringOrOwned` fields.
fn build_fields(
    original: &str,
    cs: CaseSensitivity,
) -> Result<(SubstringOrOwned, SubstringOrOwned)> {
    let with_original = |kind: crate::ErrorKind| kind.into_error(String::from(original));

    let cs_normalized = normalize_cs(original).map_err(&with_original)?;
    let normalized = match cs {
        CaseSensitivity::Sensitive => SubstringOrOwned::new(&cs_normalized, original),
        CaseSensitivity::Insensitive => {
            SubstringOrOwned::new(&normalize_ci_from_normalized_cs(&cs_normalized), original)
        }
    };
    let os_str = os_compatible_from_normalized_cs(&cs_normalized).map_err(&with_original)?;
    let os_compatible = SubstringOrOwned::new(&os_str, original);
    Ok((normalized, os_compatible))
}

/// Case-sensitive path element (compile-time case sensitivity).
///
/// Alias for `PathElementGeneric<'a, CaseSensitive>`. Implements [`Hash`](core::hash::Hash).
pub type PathElementCS<'a> = PathElementGeneric<'a, CaseSensitive>;

/// Case-insensitive path element (compile-time case sensitivity).
///
/// Alias for `PathElementGeneric<'a, CaseInsensitive>`. Implements [`Hash`](core::hash::Hash).
pub type PathElementCI<'a> = PathElementGeneric<'a, CaseInsensitive>;

/// Path element with runtime-selected case sensitivity.
///
/// Alias for `PathElementGeneric<'a, CaseSensitivity>`. Does **not** implement
/// [`Hash`](core::hash::Hash) because elements with different sensitivities must not
/// share a hash map.
pub type PathElement<'a> = PathElementGeneric<'a, CaseSensitivity>;

/// A validated, normalized single path element.
///
/// `PathElementGeneric` takes a raw path element name, validates it (rejecting empty
/// strings, `.`, `..`, and `/`), normalizes it through a Unicode normalization pipeline,
/// and computes an OS-compatible presentation form. All three views -- original,
/// normalized, and OS-compatible -- are accessible without re-computation.
///
/// The type parameter `S` controls case sensitivity:
/// - [`CaseSensitive`] -- compile-time case-sensitive (alias: [`PathElementCS`]).
/// - [`CaseInsensitive`] -- compile-time case-insensitive (alias: [`PathElementCI`]).
/// - [`CaseSensitivity`] -- runtime-selected (alias: [`PathElement`]).
///
/// Equality, ordering, and hashing are based on the **normalized** form, so two
/// `PathElementGeneric` values with different originals but the same normalized form
/// are considered equal.
///
/// Where possible, the normalized and OS-compatible forms borrow from the original
/// string to avoid allocation.
#[derive(Clone)]
pub struct PathElementGeneric<'a, S> {
    original: Cow<'a, str>,
    /// Relative to `original`.
    normalized: SubstringOrOwned,
    /// Relative to `original`.
    os_compatible: SubstringOrOwned,
    case_sensitivity: S,
}

impl<S: core::fmt::Debug> core::fmt::Debug for PathElementGeneric<'_, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PathElement")
            .field("original", &self.original())
            .field("normalized", &self.normalized())
            .field("os_compatible", &self.os_compatible())
            .field("case_sensitivity", &self.case_sensitivity)
            .finish()
    }
}

/// Compares by `normalized()`.
///
/// # Panics
///
/// Panics if `self` and `other` have different [`CaseSensitivity`] values.
/// Use [`PartialOrd`] for a non-panicking comparison that returns `None` on mismatch.
impl<'a, S1, S2> PartialEq<PathElementGeneric<'a, S2>> for PathElementGeneric<'_, S1>
where
    for<'s> CaseSensitivity: From<&'s S1> + From<&'s S2>,
{
    fn eq(&self, other: &PathElementGeneric<'a, S2>) -> bool {
        assert_eq!(
            CaseSensitivity::from(&self.case_sensitivity),
            CaseSensitivity::from(&other.case_sensitivity),
            "comparing PathElements with different case sensitivity"
        );
        self.normalized() == other.normalized()
    }
}

/// See [`PartialEq`] for panicking behavior on case sensitivity mismatch.
impl<S> Eq for PathElementGeneric<'_, S> where for<'s> CaseSensitivity: From<&'s S> {}

/// Compares by `normalized()`. Returns `None` if the two elements have
/// different `CaseSensitivity` values.
impl<'a, S1, S2> PartialOrd<PathElementGeneric<'a, S2>> for PathElementGeneric<'_, S1>
where
    for<'s> CaseSensitivity: From<&'s S1> + From<&'s S2>,
{
    fn partial_cmp(&self, other: &PathElementGeneric<'a, S2>) -> Option<core::cmp::Ordering> {
        if CaseSensitivity::from(&self.case_sensitivity)
            != CaseSensitivity::from(&other.case_sensitivity)
        {
            return None;
        }
        Some(self.normalized().cmp(other.normalized()))
    }
}

/// Compares by `normalized()`.
///
/// # Panics
///
/// Panics if `self` and `other` have different [`CaseSensitivity`] values.
/// This can only happen with the runtime-dynamic [`PathElement`] type alias.
/// The typed aliases [`PathElementCS`] and [`PathElementCI`] always have
/// matching sensitivity, so they never panic.
impl<S> Ord for PathElementGeneric<'_, S>
where
    for<'s> CaseSensitivity: From<&'s S>,
{
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        assert_eq!(
            CaseSensitivity::from(&self.case_sensitivity),
            CaseSensitivity::from(&other.case_sensitivity),
            "comparing PathElements with different case sensitivity"
        );
        self.normalized().cmp(other.normalized())
    }
}

/// Hashes by `normalized()`, consistent with `PartialEq`. Only
/// implemented for typed variants, not `PathElement`.
impl core::hash::Hash for PathElementCS<'_> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.normalized().hash(state);
    }
}

/// Hashes by `normalized()`, consistent with `PartialEq`. Only
/// implemented for typed variants, not `PathElement`.
impl core::hash::Hash for PathElementCI<'_> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.normalized().hash(state);
    }
}

impl<'a> PathElementCS<'a> {
    /// Creates a new case-sensitive path element from a string.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid (empty, `.`, `..`,
    /// or contains `/`).
    ///
    /// ```
    /// # use normalized_path::PathElementCS;
    /// let pe = PathElementCS::new("Hello.txt")?;
    /// assert_eq!(pe.normalized(), "Hello.txt"); // case preserved
    /// # Ok::<(), normalized_path::Error>(())
    /// ```
    pub fn new(original: impl Into<Cow<'a, str>>) -> Result<Self> {
        Self::with_case_sensitivity(original, CaseSensitive)
    }

    /// Creates a new case-sensitive path element from a byte slice.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid (empty, `.`, `..`,
    /// or contains `/`).
    pub fn from_bytes(original: impl Into<Cow<'a, [u8]>>) -> Result<Self> {
        Self::from_bytes_with_case_sensitivity(original, CaseSensitive)
    }

    /// Creates a new case-sensitive path element from an OS string.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid (empty, `.`, `..`,
    /// or contains `/`).
    #[cfg(feature = "std")]
    pub fn from_os_str(original: impl Into<Cow<'a, OsStr>>) -> Result<Self> {
        Self::from_os_str_with_case_sensitivity(original, CaseSensitive)
    }
}

impl<'a> PathElementCI<'a> {
    /// Creates a new case-insensitive path element from a string.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid (empty, `.`, `..`,
    /// or contains `/`).
    ///
    /// ```
    /// # use normalized_path::PathElementCI;
    /// let pe = PathElementCI::new("Hello.txt")?;
    /// assert_eq!(pe.normalized(), "hello.txt"); // case-folded
    /// # Ok::<(), normalized_path::Error>(())
    /// ```
    pub fn new(original: impl Into<Cow<'a, str>>) -> Result<Self> {
        Self::with_case_sensitivity(original, CaseInsensitive)
    }

    /// Creates a new case-insensitive path element from a byte slice.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid (empty, `.`, `..`,
    /// or contains `/`).
    pub fn from_bytes(original: impl Into<Cow<'a, [u8]>>) -> Result<Self> {
        Self::from_bytes_with_case_sensitivity(original, CaseInsensitive)
    }

    /// Creates a new case-insensitive path element from an OS string.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid (empty, `.`, `..`,
    /// or contains `/`).
    #[cfg(feature = "std")]
    pub fn from_os_str(original: impl Into<Cow<'a, OsStr>>) -> Result<Self> {
        Self::from_os_str_with_case_sensitivity(original, CaseInsensitive)
    }
}

impl<'a> PathElementGeneric<'a, CaseSensitivity> {
    /// Creates a new path element with runtime-selected case sensitivity.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid.
    pub fn new(
        original: impl Into<Cow<'a, str>>,
        case_sensitivity: impl Into<CaseSensitivity>,
    ) -> Result<Self> {
        Self::with_case_sensitivity(original, case_sensitivity)
    }

    /// Creates a new path element from a byte slice with runtime-selected case sensitivity.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid (empty, `.`, `..`,
    /// or contains `/`).
    pub fn from_bytes(
        original: impl Into<Cow<'a, [u8]>>,
        case_sensitivity: impl Into<CaseSensitivity>,
    ) -> Result<Self> {
        Self::from_bytes_with_case_sensitivity(original, case_sensitivity)
    }

    /// Creates a new path element from an OS string with runtime-selected case sensitivity.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid (empty, `.`, `..`,
    /// or contains `/`).
    #[cfg(feature = "std")]
    pub fn from_os_str(
        original: impl Into<Cow<'a, OsStr>>,
        case_sensitivity: impl Into<CaseSensitivity>,
    ) -> Result<Self> {
        Self::from_os_str_with_case_sensitivity(original, case_sensitivity)
    }

    /// Convenience constructor for a case-sensitive `PathElement`.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid.
    pub fn new_cs(original: impl Into<Cow<'a, str>>) -> Result<Self> {
        Self::with_case_sensitivity(original, CaseSensitive)
    }

    /// Convenience constructor for a case-insensitive `PathElement`.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid.
    pub fn new_ci(original: impl Into<Cow<'a, str>>) -> Result<Self> {
        Self::with_case_sensitivity(original, CaseInsensitive)
    }

    /// Convenience constructor for a case-sensitive `PathElement` from bytes.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid.
    pub fn from_bytes_cs(original: impl Into<Cow<'a, [u8]>>) -> Result<Self> {
        Self::from_bytes_with_case_sensitivity(original, CaseSensitive)
    }

    /// Convenience constructor for a case-insensitive `PathElement` from bytes.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid.
    pub fn from_bytes_ci(original: impl Into<Cow<'a, [u8]>>) -> Result<Self> {
        Self::from_bytes_with_case_sensitivity(original, CaseInsensitive)
    }

    /// Convenience constructor for a case-sensitive `PathElement` from an OS string.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid.
    #[cfg(feature = "std")]
    pub fn from_os_str_cs(original: impl Into<Cow<'a, OsStr>>) -> Result<Self> {
        Self::from_os_str_with_case_sensitivity(original, CaseSensitive)
    }

    /// Convenience constructor for a case-insensitive `PathElement` from an OS string.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid.
    #[cfg(feature = "std")]
    pub fn from_os_str_ci(original: impl Into<Cow<'a, OsStr>>) -> Result<Self> {
        Self::from_os_str_with_case_sensitivity(original, CaseInsensitive)
    }
}

impl<'a, S> PathElementGeneric<'a, S>
where
    for<'s> CaseSensitivity: From<&'s S>,
{
    /// Creates a new path element from a byte slice with an explicit case-sensitivity
    /// marker.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// This is the most general byte-input constructor. The typed aliases
    /// ([`PathElementCS::from_bytes()`], [`PathElementCI::from_bytes()`]) and the
    /// runtime-dynamic constructors delegate to this method.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid (empty, `.`, `..`,
    /// or contains `/`).
    pub fn from_bytes_with_case_sensitivity(
        original: impl Into<Cow<'a, [u8]>>,
        case_sensitivity: impl Into<S>,
    ) -> Result<Self> {
        let cow_str = match original.into() {
            Cow::Borrowed(b) => String::from_utf8_lossy(b),
            // TODO: replace with `Cow::Owned(String::from_utf8_lossy_owned(v))` once stable (rust#129436).
            Cow::Owned(v) => match String::from_utf8_lossy(&v) {
                // SAFETY: `String::from_utf8_lossy()` returned Borrowed, so the bytes are valid UTF-8.
                Cow::Borrowed(_) => unsafe { Cow::Owned(String::from_utf8_unchecked(v)) },
                Cow::Owned(s) => Cow::Owned(s),
            },
        };
        Self::with_case_sensitivity(cow_str, case_sensitivity)
    }

    /// Creates a new path element from an OS string with an explicit case-sensitivity
    /// marker.
    ///
    /// Invalid UTF-8 is accepted; see
    /// [Normalization pipeline](crate#normalization-pipeline) step 0.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid (empty, `.`, `..`,
    /// or contains `/`).
    #[cfg(feature = "std")]
    pub fn from_os_str_with_case_sensitivity(
        original: impl Into<Cow<'a, OsStr>>,
        case_sensitivity: impl Into<S>,
    ) -> Result<Self> {
        let cow_bytes: Cow<'a, [u8]> = match original.into() {
            Cow::Borrowed(os) => Cow::Borrowed(os.as_encoded_bytes()),
            Cow::Owned(os) => Cow::Owned(os.into_encoded_bytes()),
        };
        Self::from_bytes_with_case_sensitivity(cow_bytes, case_sensitivity)
    }

    /// Creates a new path element with an explicit case-sensitivity marker.
    ///
    /// This is the most general string constructor. The typed aliases
    /// ([`PathElementCS::new()`], [`PathElementCI::new()`]) and the runtime-dynamic
    /// constructors delegate to this method.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::Error) if the name is invalid (empty after
    /// normalization, `.`, `..`, or contains `/`).
    pub fn with_case_sensitivity(
        original: impl Into<Cow<'a, str>>,
        case_sensitivity: impl Into<S>,
    ) -> Result<Self> {
        let original = original.into();
        let case_sensitivity = case_sensitivity.into();
        let cs = CaseSensitivity::from(&case_sensitivity);
        let (normalized, os_compatible) = build_fields(&original, cs)?;
        Ok(Self {
            original,
            normalized,
            os_compatible,
            case_sensitivity,
        })
    }

    /// Returns the case sensitivity of this path element as a [`CaseSensitivity`] enum.
    pub fn case_sensitivity(&self) -> CaseSensitivity {
        CaseSensitivity::from(&self.case_sensitivity)
    }
}

impl<'a, S> PathElementGeneric<'a, S> {
    /// Returns the original input string, before any normalization.
    ///
    /// ```
    /// # use normalized_path::PathElementCS;
    /// let pe = PathElementCS::new("  Hello.txt  ")?;
    /// assert_eq!(pe.original(), "  Hello.txt  ");
    /// assert_eq!(pe.normalized(), "Hello.txt");
    /// # Ok::<(), normalized_path::Error>(())
    /// ```
    pub fn original(&self) -> &str {
        &self.original
    }

    /// Consumes `self` and returns the original input string.
    pub fn into_original(self) -> Cow<'a, str> {
        self.original
    }

    /// Returns `true` if the normalized form is identical to the original.
    ///
    /// When this returns `true`, no allocation was needed for the normalized form.
    ///
    /// ```
    /// # use normalized_path::PathElementCS;
    /// assert!(PathElementCS::new("hello.txt")?.is_normalized());
    /// assert!(!PathElementCS::new("  hello.txt  ")?.is_normalized());
    /// # Ok::<(), normalized_path::Error>(())
    /// ```
    pub fn is_normalized(&self) -> bool {
        self.normalized.is_identity(&self.original)
    }

    /// Returns the normalized form of the path element name.
    ///
    /// This is the canonical representation used for equality comparisons, ordering,
    /// and hashing. In case-sensitive mode it is NFC-normalized; in case-insensitive
    /// mode it is additionally case-folded.
    pub fn normalized(&self) -> &str {
        self.normalized.as_ref(&self.original)
    }

    /// Consumes `self` and returns the normalized form as a [`Cow`].
    ///
    /// Returns `Cow::Borrowed` when the normalized form is a substring of the original
    /// and the original was itself borrowed.
    pub fn into_normalized(self) -> Cow<'a, str> {
        self.normalized.into_cow(self.original)
    }

    /// Returns `true` if the OS-compatible form is identical to the original.
    pub fn is_os_compatible(&self) -> bool {
        self.os_compatible.is_identity(&self.original)
    }

    /// Returns the OS-compatible presentation form of the path element name.
    ///
    /// ```
    /// # use normalized_path::PathElementCS;
    /// let pe = PathElementCS::new("hello.txt")?;
    /// assert_eq!(pe.os_compatible(), "hello.txt");
    /// # Ok::<(), normalized_path::Error>(())
    /// ```
    pub fn os_compatible(&self) -> &str {
        self.os_compatible.as_ref(&self.original)
    }

    /// Consumes `self` and returns the OS-compatible form as a [`Cow<str>`](Cow).
    pub fn into_os_compatible(self) -> Cow<'a, str> {
        self.os_compatible.into_cow(self.original)
    }

    /// Returns the OS-compatible form as an [`OsStr`] reference.
    #[cfg(feature = "std")]
    pub fn os_str(&self) -> &OsStr {
        OsStr::new(self.os_compatible())
    }

    /// Consumes `self` and returns the OS-compatible form as a [`Cow<OsStr>`](Cow).
    #[cfg(feature = "std")]
    pub fn into_os_str(self) -> Cow<'a, OsStr> {
        match self.into_os_compatible() {
            Cow::Borrowed(s) => Cow::Borrowed(OsStr::new(s)),
            Cow::Owned(s) => Cow::Owned(OsString::from(s)),
        }
    }

    /// Returns `true` if the original string is borrowed (not owned).
    ///
    /// ```
    /// # use std::borrow::Cow;
    /// # use normalized_path::PathElementCS;
    /// let borrowed = PathElementCS::new(Cow::Borrowed("hello"))?;
    /// assert!(borrowed.is_borrowed());
    ///
    /// let owned = PathElementCS::new(Cow::<str>::Owned("hello".into()))?;
    /// assert!(!owned.is_borrowed());
    /// # Ok::<(), normalized_path::Error>(())
    /// ```
    pub fn is_borrowed(&self) -> bool {
        matches!(self.original, Cow::Borrowed(_))
    }

    /// Returns `true` if the original string is owned (not borrowed).
    pub fn is_owned(&self) -> bool {
        matches!(self.original, Cow::Owned(_))
    }

    /// Consumes `self` and returns an equivalent `PathElementGeneric` with a `'static`
    /// lifetime by cloning the original string if it was borrowed.
    pub fn into_owned(self) -> PathElementGeneric<'static, S> {
        PathElementGeneric {
            original: Cow::Owned(self.original.into_owned()),
            normalized: self.normalized,
            os_compatible: self.os_compatible,
            case_sensitivity: self.case_sensitivity,
        }
    }
}

// --- Conversions ---

/// Converts a compile-time case-sensitive element into a runtime-dynamic [`PathElement`].
impl<'a> From<PathElementCS<'a>> for PathElement<'a> {
    fn from(pe: PathElementCS<'a>) -> Self {
        PathElementGeneric {
            original: pe.original,
            normalized: pe.normalized,
            os_compatible: pe.os_compatible,
            case_sensitivity: CaseSensitivity::Sensitive,
        }
    }
}

/// Converts a compile-time case-insensitive element into a runtime-dynamic [`PathElement`].
impl<'a> From<PathElementCI<'a>> for PathElement<'a> {
    fn from(pe: PathElementCI<'a>) -> Self {
        PathElementGeneric {
            original: pe.original,
            normalized: pe.normalized,
            os_compatible: pe.os_compatible,
            case_sensitivity: CaseSensitivity::Insensitive,
        }
    }
}

/// Attempts to convert a runtime-dynamic [`PathElement`] into a [`PathElementCS`].
///
/// Succeeds if the element is case-sensitive. On failure, returns the element
/// re-wrapped as a [`PathElementCI`] in the `Err` variant (no data is lost).
impl<'a> TryFrom<PathElement<'a>> for PathElementCS<'a> {
    type Error = PathElementCI<'a>;

    fn try_from(pe: PathElement<'a>) -> core::result::Result<Self, Self::Error> {
        if pe.case_sensitivity == CaseSensitivity::Sensitive {
            Ok(PathElementGeneric {
                original: pe.original,
                normalized: pe.normalized,
                os_compatible: pe.os_compatible,
                case_sensitivity: CaseSensitive,
            })
        } else {
            Err(PathElementGeneric {
                original: pe.original,
                normalized: pe.normalized,
                os_compatible: pe.os_compatible,
                case_sensitivity: CaseInsensitive,
            })
        }
    }
}

/// Attempts to convert a runtime-dynamic [`PathElement`] into a [`PathElementCI`].
///
/// Succeeds if the element is case-insensitive. On failure, returns the element
/// re-wrapped as a [`PathElementCS`] in the `Err` variant (no data is lost).
impl<'a> TryFrom<PathElement<'a>> for PathElementCI<'a> {
    type Error = PathElementCS<'a>;

    fn try_from(pe: PathElement<'a>) -> core::result::Result<Self, Self::Error> {
        if pe.case_sensitivity == CaseSensitivity::Insensitive {
            Ok(PathElementGeneric {
                original: pe.original,
                normalized: pe.normalized,
                os_compatible: pe.os_compatible,
                case_sensitivity: CaseInsensitive,
            })
        } else {
            Err(PathElementGeneric {
                original: pe.original,
                normalized: pe.normalized,
                os_compatible: pe.os_compatible,
                case_sensitivity: CaseSensitive,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use alloc::string::ToString;
    use alloc::vec::Vec;

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{PathElement, PathElementCI, PathElementCS};
    use crate::case_sensitivity::{CaseInsensitive, CaseSensitive, CaseSensitivity};
    use crate::normalize::{normalize_ci_from_normalized_cs, normalize_cs};
    use crate::os::os_compatible_from_normalized_cs;

    // --- PathElement ---

    // CS "H\tllo": original="H\tllo", normalized="H␉llo", os_compatible="H␉llo"
    // original != normalized (control mapping), os_compatible == normalized on all platforms.
    #[test]
    fn path_element_cs_matches_freestanding() {
        let input = "H\tllo";
        let pe = PathElementCS::new(Cow::Borrowed(input)).unwrap();
        assert_eq!(pe.original(), input);
        assert_eq!(pe.normalized(), normalize_cs(input).unwrap().as_ref());
        assert_eq!(
            pe.os_compatible(),
            os_compatible_from_normalized_cs(&normalize_cs(input).unwrap())
                .unwrap()
                .as_ref()
        );
    }

    // CI "H\tllo": original="H\tllo", os_compatible="H␉llo", normalized="h␉llo"
    // All three differ on all platforms.
    #[test]
    fn path_element_ci_matches_freestanding() {
        let input = "H\tllo";
        let pe = PathElementCI::new(Cow::Borrowed(input)).unwrap();
        assert_eq!(pe.original(), "H\tllo");
        assert_eq!(pe.normalized(), "h\u{2409}llo");
        assert_eq!(pe.os_compatible(), "H\u{2409}llo");
    }

    // CS "nul.e\u{0301}": normalized="nul.é" (NFC), os_compatible is platform-dependent.
    // Windows: "\u{FF4E}ul.é" (reserved name), Apple: "nul.e\u{0301}" (NFD), Linux: "nul.é".
    #[test]
    fn path_element_cs_os_compatible_platform_dependent() {
        let input = "nul.e\u{0301}";
        let pe = PathElementCS::new(input).unwrap();
        assert_eq!(pe.original(), "nul.e\u{0301}");
        assert_eq!(pe.normalized(), "nul.\u{00E9}");
        #[cfg(target_os = "windows")]
        assert_eq!(pe.os_compatible(), "\u{FF4E}ul.\u{00E9}");
        #[cfg(target_vendor = "apple")]
        assert_eq!(pe.os_compatible(), "nul.e\u{0301}");
        #[cfg(not(any(target_os = "windows", target_vendor = "apple")))]
        assert_eq!(pe.os_compatible(), "nul.\u{00E9}");
    }

    #[test]
    fn path_element_cs_nfc_matches_freestanding() {
        let input = "e\u{0301}.txt";
        let pe = PathElementCS::new(Cow::Borrowed(input)).unwrap();
        assert_eq!(pe.normalized(), normalize_cs(input).unwrap().as_ref());
    }

    #[test]
    fn path_element_ci_casefold_matches_freestanding() {
        let input = "Hello.txt";
        let pe = PathElementCI::new(Cow::Borrowed(input)).unwrap();
        let cs = normalize_cs(input).unwrap();
        assert_eq!(
            pe.normalized(),
            normalize_ci_from_normalized_cs(&cs).as_ref()
        );
    }

    #[test]
    fn path_element_cs_normalized_borrows_from_original() {
        let input = "hello.txt";
        let pe = PathElementCS::new(Cow::Borrowed(input)).unwrap();
        assert_eq!(pe.normalized(), "hello.txt");
        assert!(core::ptr::eq(pe.normalized().as_ptr(), input.as_ptr()));
    }

    #[test]
    fn path_element_cs_into_normalized_borrows() {
        let input = "hello.txt";
        let pe = PathElementCS::new(Cow::Borrowed(input)).unwrap();
        let norm = pe.into_normalized();
        assert!(matches!(norm, Cow::Borrowed(_)));
        assert_eq!(norm, "hello.txt");
    }

    #[test]
    fn path_element_cs_into_os_compatible_borrows() {
        let input = "hello.txt";
        let pe = PathElementCS::new(Cow::Borrowed(input)).unwrap();
        let pres = pe.into_os_compatible();
        assert!(matches!(pres, Cow::Borrowed(_)));
        assert_eq!(pres.as_ref(), "hello.txt");
    }

    #[test]
    fn path_element_ci_normalized_borrows_when_already_folded() {
        let input = "hello.txt";
        let pe = PathElementCI::new(Cow::Borrowed(input)).unwrap();
        assert!(core::ptr::eq(pe.normalized().as_ptr(), input.as_ptr()));
    }

    #[test]
    fn path_element_ci_into_normalized_borrows_when_already_folded() {
        let input = "hello.txt";
        let pe = PathElementCI::new(Cow::Borrowed(input)).unwrap();
        let norm = pe.into_normalized();
        assert!(matches!(norm, Cow::Borrowed(_)));
        assert_eq!(norm, "hello.txt");
    }

    #[test]
    fn path_element_ci_into_os_compatible_borrows_when_already_folded() {
        let input = "hello.txt";
        let pe = PathElementCI::new(Cow::Borrowed(input)).unwrap();
        let pres = pe.into_os_compatible();
        assert!(matches!(pres, Cow::Borrowed(_)));
        assert_eq!(pres.as_ref(), "hello.txt");
    }

    #[test]
    fn path_element_cs_trimmed_borrows_suffix() {
        let input = "   hello.txt";
        let pe = PathElementCS::new(Cow::Borrowed(input)).unwrap();
        assert_eq!(pe.normalized(), "hello.txt");
        assert!(core::ptr::eq(pe.normalized().as_ptr(), input[3..].as_ptr()));
    }

    #[test]
    fn path_element_into_original_returns_original() {
        let input = "  Hello.txt  ";
        let pe = PathElementCS::new(Cow::Borrowed(input)).unwrap();
        let orig = pe.into_original();
        assert!(matches!(orig, Cow::Borrowed(_)));
        assert_eq!(orig, input);
    }

    #[test]
    fn path_element_is_normalized_when_unchanged() {
        let pe = PathElementCS::new("hello.txt").unwrap();
        assert!(pe.is_normalized());
    }

    #[test]
    fn path_element_is_not_normalized_when_trimmed() {
        let pe = PathElementCS::new("  hello.txt  ").unwrap();
        assert!(!pe.is_normalized());
    }

    #[test]
    fn path_element_is_not_normalized_when_trailing_whitespace_trimmed() {
        // Trailing-only trim produces Substring(0, shorter_len) — a prefix, not identity.
        let pe = PathElementCS::new("hello.txt  ").unwrap();
        assert!(!pe.is_normalized());
        assert_eq!(pe.normalized(), "hello.txt");
    }

    #[test]
    fn path_element_is_not_normalized_when_casefolded() {
        let pe = PathElementCI::new("Hello.txt").unwrap();
        assert!(!pe.is_normalized());
    }

    #[test]
    fn path_element_ci_is_normalized_when_already_folded() {
        let pe = PathElementCI::new("hello.txt").unwrap();
        assert!(pe.is_normalized());
    }

    #[test]
    fn path_element_is_os_compatible_ascii() {
        let pe = PathElementCS::new("hello.txt").unwrap();
        assert!(pe.is_os_compatible());
    }

    #[test]
    fn path_element_is_not_os_compatible_trailing_whitespace_ci() {
        // CI: os_compatible is relative to original. Trailing trim produces
        // Substring(0, shorter_len) — a prefix, not identity.
        let pe = PathElementCI::new("hello.txt  ").unwrap();
        assert!(!pe.is_os_compatible());
    }

    #[test]
    fn path_element_is_not_os_compatible_trailing_whitespace_cs() {
        // CS: both normalized and os_compatible must be identity.
        // Trailing trim makes normalized a prefix of original.
        let pe = PathElementCS::new("hello.txt  ").unwrap();
        assert!(!pe.is_os_compatible());
    }

    #[test]
    fn path_element_is_os_compatible_nfc_input() {
        // NFC input "é" stays NFC after normalization. On Apple, os_compatible
        // converts to NFD "e\u{0301}", so original != os_compatible.
        // On non-Apple, os_compatible == normalized == original.
        let pe = PathElementCS::new("\u{00E9}.txt").unwrap();
        #[cfg(target_vendor = "apple")]
        assert!(!pe.is_os_compatible());
        #[cfg(not(target_vendor = "apple"))]
        assert!(pe.is_os_compatible());
    }

    #[test]
    fn path_element_is_os_compatible_nfd_input() {
        // NFD input "e\u{0301}" normalizes to NFC "é", so original != os_compatible
        // on non-Apple. On Apple, os_compatible converts back to NFD, matching the original.
        let pe = PathElementCS::new("e\u{0301}.txt").unwrap();
        #[cfg(target_vendor = "apple")]
        assert!(pe.is_os_compatible());
        #[cfg(not(target_vendor = "apple"))]
        assert!(!pe.is_os_compatible());
    }

    #[test]
    fn path_element_is_os_compatible_nfd_input_ci() {
        // Regression: NFD input in CI mode. cs_normalized is NFC (owned allocation),
        // then os_compatible_from_normalized_cs may convert back to NFD. The result string-equals
        // original but is stored as Owned (pointer doesn't overlap original).
        // The identity fast-path misses this; string comparison fallback catches it.
        let pe = PathElementCI::new("e\u{0301}.txt").unwrap();
        #[cfg(target_vendor = "apple")]
        assert!(pe.is_os_compatible());
        #[cfg(not(target_vendor = "apple"))]
        assert!(!pe.is_os_compatible());
    }

    #[test]
    fn path_element_is_os_compatible_nfc_input_ci() {
        // NFC input in CI mode. cs_normalized borrows from original (already NFC).
        // On Apple, os_compatible converts to NFD, so original != os_compatible.
        let pe = PathElementCI::new("\u{00E9}.txt").unwrap();
        #[cfg(target_vendor = "apple")]
        assert!(!pe.is_os_compatible());
        #[cfg(not(target_vendor = "apple"))]
        assert!(pe.is_os_compatible());
    }

    #[test]
    fn path_element_is_not_os_compatible_reserved_on_windows() {
        let pe = PathElementCS::new("nul.txt").unwrap();
        #[cfg(target_os = "windows")]
        assert!(!pe.is_os_compatible());
        #[cfg(not(target_os = "windows"))]
        assert!(pe.is_os_compatible());
    }

    #[test]
    fn path_element_borrowed_is_borrowed() {
        let pe = PathElementCS::new(Cow::Borrowed("hello.txt")).unwrap();
        assert!(pe.is_borrowed());
        assert!(!pe.is_owned());
    }

    #[test]
    fn path_element_owned_is_owned() {
        let pe = PathElementCS::new(Cow::Owned("hello.txt".to_string())).unwrap();
        assert!(pe.is_owned());
        assert!(!pe.is_borrowed());
    }

    #[test]
    fn path_element_into_owned_is_owned() {
        let pe = PathElementCS::new(Cow::Borrowed("hello.txt")).unwrap();
        let owned = pe.into_owned();
        assert!(owned.is_owned());
    }

    #[test]
    fn path_element_into_owned_preserves_values() {
        let input = "H\tllo";
        let pe = PathElementCI::new(Cow::Borrowed(input)).unwrap();
        let owned = pe.into_owned();
        assert_eq!(owned.original(), "H\tllo");
        assert_eq!(owned.normalized(), "h\u{2409}llo");
        assert_eq!(owned.os_compatible(), "H\u{2409}llo");
    }

    #[test]
    fn path_element_rejects_invalid() {
        assert!(PathElementCS::new("").is_err());
        assert!(PathElementCS::new(".").is_err());
        assert!(PathElementCS::new("..").is_err());
        assert!(PathElementCS::new("a/b").is_err());
        assert!(PathElementCS::new("\0").is_err());
        assert!(PathElementCS::new("a\0b").is_err());
    }

    // --- PartialEq / Eq ---

    #[test]
    fn path_element_eq_same_cs() {
        let a = PathElementCS::new("hello.txt").unwrap();
        let b = PathElementCS::new("hello.txt").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn path_element_eq_different_original_same_normalized_cs() {
        let a = PathElementCS::new("  hello.txt  ").unwrap();
        let b = PathElementCS::new("hello.txt").unwrap();
        assert_ne!(a.original(), b.original());
        assert_eq!(a, b);
    }

    #[test]
    fn path_element_ne_different_case_cs() {
        let a = PathElementCS::new("Hello.txt").unwrap();
        let b = PathElementCS::new("hello.txt").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn path_element_eq_different_case_ci() {
        let a = PathElementCI::new("Hello.txt").unwrap();
        let b = PathElementCI::new("hello.txt").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn path_element_eq_nfc_nfd_cs() {
        let a = PathElementCS::new("\u{00E9}.txt").unwrap();
        let b = PathElementCS::new("e\u{0301}.txt").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn path_element_eq_cross_lifetime() {
        let owned = PathElementCS::new("hello.txt").unwrap().into_owned();
        let input = "hello.txt";
        let borrowed = PathElementCS::new(Cow::Borrowed(input)).unwrap();
        assert_eq!(owned, borrowed);
        assert_eq!(borrowed, owned);
    }

    #[test]
    #[should_panic(expected = "different case sensitivity")]
    fn path_element_eq_panics_on_mixed_dynamic_case_sensitivity() {
        let a = PathElement::new("hello", CaseSensitive).unwrap();
        let b = PathElement::new("hello", CaseInsensitive).unwrap();
        let _ = a == b;
    }

    // --- PartialOrd / Ord ---

    #[test]
    fn path_element_ord_alphabetical_cs() {
        let a = PathElementCS::new("apple").unwrap();
        let b = PathElementCS::new("banana").unwrap();
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn path_element_ord_equal_cs() {
        let a = PathElementCS::new("hello").unwrap();
        let b = PathElementCS::new("hello").unwrap();
        assert_eq!(a.cmp(&b), core::cmp::Ordering::Equal);
    }

    #[test]
    fn path_element_ord_case_ci() {
        let a = PathElementCI::new("Apple").unwrap();
        let b = PathElementCI::new("apple").unwrap();
        assert_eq!(a.cmp(&b), core::cmp::Ordering::Equal);
    }

    #[test]
    fn path_element_partial_ord_cross_lifetime() {
        let owned = PathElementCS::new("apple").unwrap().into_owned();
        let input = "banana";
        let borrowed = PathElementCS::new(Cow::Borrowed(input)).unwrap();
        assert!(owned < borrowed);
    }

    #[test]
    fn path_element_partial_ord_none_on_mixed_dynamic_case_sensitivity() {
        let a = PathElement::new("hello", CaseSensitive).unwrap();
        let b = PathElement::new("hello", CaseInsensitive).unwrap();
        assert_eq!(a.partial_cmp(&b), None);
    }

    #[test]
    fn path_element_ord_sortable() {
        let mut elems: Vec<_> = ["cherry", "apple", "banana"]
            .iter()
            .map(|s| PathElementCS::new(Cow::Borrowed(*s)).unwrap())
            .collect();
        elems.sort();
        let names: Vec<_> = elems.iter().map(PathElementCS::normalized).collect();
        assert_eq!(names, &["apple", "banana", "cherry"]);
    }

    #[test]
    fn path_element_ord_ci_sortable() {
        let mut elems: Vec<_> = ["Cherry", "apple", "BANANA"]
            .iter()
            .map(|s| PathElementCI::new(Cow::Borrowed(*s)).unwrap())
            .collect();
        elems.sort();
        let names: Vec<_> = elems.iter().map(PathElementCI::normalized).collect();
        assert_eq!(names, &["apple", "banana", "cherry"]);
    }

    // --- Conversions ---

    #[test]
    fn from_cs_into_dynamic() {
        let pe = PathElementCS::new("hello").unwrap();
        let dyn_pe: PathElement<'_> = pe.into();
        assert_eq!(dyn_pe.case_sensitivity(), CaseSensitivity::Sensitive);
        assert_eq!(dyn_pe.normalized(), "hello");
    }

    #[test]
    fn from_ci_into_dynamic() {
        let pe = PathElementCI::new("Hello").unwrap();
        let dyn_pe: PathElement<'_> = pe.into();
        assert_eq!(dyn_pe.case_sensitivity(), CaseSensitivity::Insensitive);
        assert_eq!(dyn_pe.normalized(), "hello");
    }

    #[test]
    fn try_from_dynamic_to_cs() {
        let pe = PathElement::new("hello", CaseSensitive).unwrap();
        let cs_pe: PathElementCS<'_> = pe.try_into().unwrap();
        assert_eq!(cs_pe.normalized(), "hello");
    }

    #[test]
    fn try_from_dynamic_to_cs_wrong_variant() {
        let pe = PathElement::new("Hello", CaseInsensitive).unwrap();
        let err: PathElementCI<'_> = PathElementCS::try_from(pe).unwrap_err();
        assert_eq!(err.original(), "Hello");
        assert_eq!(err.normalized(), "hello");
        assert_eq!(err.os_compatible(), "Hello");
    }

    #[test]
    fn try_from_dynamic_to_ci() {
        let pe = PathElement::new("Hello", CaseInsensitive).unwrap();
        let ci_pe: PathElementCI<'_> = pe.try_into().unwrap();
        assert_eq!(ci_pe.normalized(), "hello");
    }

    // --- PathElement convenience constructors ---

    #[test]
    fn dyn_new_cs() {
        let pe = PathElement::new_cs("Hello.txt").unwrap();
        assert_eq!(pe.case_sensitivity(), CaseSensitivity::Sensitive);
        assert_eq!(pe.normalized(), "Hello.txt");
    }

    #[test]
    fn dyn_new_ci() {
        let pe = PathElement::new_ci("Hello.txt").unwrap();
        assert_eq!(pe.case_sensitivity(), CaseSensitivity::Insensitive);
        assert_eq!(pe.normalized(), "hello.txt");
    }

    #[test]
    fn dyn_new_cs_matches_typed() {
        let dyn_pe = PathElement::new_cs("Hello.txt").unwrap();
        let cs_pe = PathElementCS::new("Hello.txt").unwrap();
        assert_eq!(dyn_pe.normalized(), cs_pe.normalized());
        assert_eq!(dyn_pe.os_compatible(), cs_pe.os_compatible());
    }

    #[test]
    fn dyn_new_ci_matches_typed() {
        let dyn_pe = PathElement::new_ci("Hello.txt").unwrap();
        let ci_pe = PathElementCI::new("Hello.txt").unwrap();
        assert_eq!(dyn_pe.normalized(), ci_pe.normalized());
        assert_eq!(dyn_pe.os_compatible(), ci_pe.os_compatible());
    }

    // --- case_sensitivity() getter ---

    #[test]
    fn case_sensitivity_cs() {
        let pe = PathElementCS::new("hello").unwrap();
        assert_eq!(pe.case_sensitivity(), CaseSensitivity::Sensitive);
    }

    #[test]
    fn case_sensitivity_ci() {
        let pe = PathElementCI::new("hello").unwrap();
        assert_eq!(pe.case_sensitivity(), CaseSensitivity::Insensitive);
    }

    #[test]
    fn case_sensitivity_dyn() {
        let cs = PathElement::new("hello", CaseSensitive).unwrap();
        let ci = PathElement::new("hello", CaseInsensitive).unwrap();
        assert_eq!(cs.case_sensitivity(), CaseSensitivity::Sensitive);
        assert_eq!(ci.case_sensitivity(), CaseSensitivity::Insensitive);
    }

    // --- PartialOrd returns None on mismatch ---

    #[test]
    fn partial_ord_dyn_same_case_sensitivity() {
        let a = PathElement::new("apple", CaseSensitive).unwrap();
        let b = PathElement::new("banana", CaseSensitive).unwrap();
        assert!(a < b);
    }

    #[test]
    fn partial_ord_dyn_none_on_mismatch() {
        let a = PathElement::new("hello", CaseSensitive).unwrap();
        let b = PathElement::new("hello", CaseInsensitive).unwrap();
        assert_eq!(a.partial_cmp(&b), None);
    }

    // --- TryFrom error returns original ---

    #[test]
    fn try_from_dynamic_to_ci_wrong_variant() {
        let pe = PathElement::new("Hello", CaseSensitive).unwrap();
        let err: PathElementCS<'_> = PathElementCI::try_from(pe).unwrap_err();
        assert_eq!(err.original(), "Hello");
        assert_eq!(err.normalized(), "Hello");
        assert_eq!(err.os_compatible(), "Hello");
    }

    // --- into_owned preserves case_sensitivity ---

    #[test]
    fn into_owned_preserves_cs_case_sensitivity() {
        let pe = PathElementCS::new("hello").unwrap();
        let owned = pe.into_owned();
        assert_eq!(owned.case_sensitivity(), CaseSensitivity::Sensitive);
    }

    #[test]
    fn into_owned_preserves_dyn_case_sensitivity() {
        let pe = PathElement::new("hello", CaseInsensitive).unwrap();
        let owned = pe.into_owned();
        assert_eq!(owned.case_sensitivity(), CaseSensitivity::Insensitive);
    }

    // --- Cross-type PartialEq ---

    #[test]
    fn eq_cs_vs_dyn_same_case_sensitivity() {
        let cs = PathElementCS::new("hello").unwrap();
        let dyn_cs = PathElement::new_cs("hello").unwrap();
        assert_eq!(cs, dyn_cs);
        assert_eq!(dyn_cs, cs);
    }

    #[test]
    fn eq_ci_vs_dyn_same_case_sensitivity() {
        let ci = PathElementCI::new("Hello").unwrap();
        let dyn_ci = PathElement::new_ci("hello").unwrap();
        assert_eq!(ci, dyn_ci);
        assert_eq!(dyn_ci, ci);
    }

    #[test]
    #[should_panic(expected = "different case sensitivity")]
    fn eq_cs_vs_ci_panics() {
        let cs = PathElementCS::new("hello").unwrap();
        let ci = PathElementCI::new("hello").unwrap();
        let _ = cs == ci;
    }

    #[test]
    #[should_panic(expected = "different case sensitivity")]
    fn eq_cs_vs_dyn_ci_panics() {
        let cs = PathElementCS::new("hello").unwrap();
        let dyn_ci = PathElement::new_ci("hello").unwrap();
        let _ = cs == dyn_ci;
    }

    // --- Cross-type PartialOrd ---

    #[test]
    fn partial_ord_cs_vs_dyn_same_case_sensitivity() {
        let cs = PathElementCS::new("apple").unwrap();
        let dyn_cs = PathElement::new_cs("banana").unwrap();
        assert!(cs < dyn_cs);
        assert!(dyn_cs > cs);
    }

    #[test]
    fn partial_ord_cs_vs_ci_none() {
        let cs = PathElementCS::new("hello").unwrap();
        let ci = PathElementCI::new("hello").unwrap();
        assert_eq!(cs.partial_cmp(&ci), None);
        assert_eq!(ci.partial_cmp(&cs), None);
    }

    #[test]
    fn partial_ord_cs_vs_dyn_ci_none() {
        let cs = PathElementCS::new("hello").unwrap();
        let dyn_ci = PathElement::new_ci("hello").unwrap();
        assert_eq!(cs.partial_cmp(&dyn_ci), None);
        assert_eq!(dyn_ci.partial_cmp(&cs), None);
    }

    // --- from_os_str / os_str / into_os_str ---

    #[cfg(feature = "std")]
    mod os_str_tests {
        use std::borrow::Cow;
        use std::ffi::{OsStr, OsString};

        #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
        use wasm_bindgen_test::wasm_bindgen_test as test;

        use crate::case_sensitivity::{CaseInsensitive, CaseSensitivity};
        use crate::path_element::{PathElement, PathElementCI, PathElementCS};

        #[test]
        fn from_os_str_borrowed_matches_new() {
            let input = OsStr::new("hello.txt");
            let from_os = PathElementCS::from_os_str(input).unwrap();
            let from_new = PathElementCS::new("hello.txt").unwrap();
            assert_eq!(from_os.original(), from_new.original());
            assert_eq!(from_os.normalized(), from_new.normalized());
            assert_eq!(from_os.os_compatible(), from_new.os_compatible());
        }

        #[test]
        fn from_os_str_owned_matches_new() {
            let input = OsString::from("Hello.txt");
            let from_os = PathElementCI::from_os_str(input).unwrap();
            let from_new = PathElementCI::new("Hello.txt").unwrap();
            assert_eq!(from_os.original(), from_new.original());
            assert_eq!(from_os.normalized(), from_new.normalized());
            assert_eq!(from_os.os_compatible(), from_new.os_compatible());
        }

        #[test]
        fn from_os_str_borrowed_preserves_borrow() {
            let input = OsStr::new("hello.txt");
            let pe = PathElementCS::from_os_str(input).unwrap();
            let orig = pe.into_original();
            assert!(matches!(orig, Cow::Borrowed(_)));
        }

        #[cfg(unix)]
        #[test]
        fn from_os_str_invalid_utf8_borrowed() {
            use std::os::unix::ffi::OsStrExt;
            let input = OsStr::from_bytes(&[0x68, 0x69, 0xFF]); // "hi" + invalid byte
            let pe = PathElementCS::from_os_str(input).unwrap();
            assert_eq!(pe.original(), "hi\u{FFFD}");
        }

        #[cfg(unix)]
        #[test]
        fn from_os_str_invalid_utf8_owned() {
            use std::os::unix::ffi::OsStrExt;
            let input = OsStr::from_bytes(&[0x68, 0x69, 0xFF]).to_os_string();
            let pe = PathElementCS::from_os_str(input).unwrap();
            assert_eq!(pe.original(), "hi\u{FFFD}");
        }

        #[cfg(windows)]
        #[test]
        fn from_os_str_invalid_utf8_borrowed() {
            use std::os::windows::ffi::OsStringExt;
            // Unpaired surrogate U+D800 encodes as 3 WTF-8 bytes, each replaced by U+FFFD
            let input = OsString::from_wide(&[0x68, 0xD800, 0x69]);
            let pe = PathElementCS::from_os_str(input.as_os_str()).unwrap();
            assert_eq!(pe.original(), "h\u{FFFD}\u{FFFD}\u{FFFD}i");
        }

        #[cfg(windows)]
        #[test]
        fn from_os_str_invalid_utf8_owned() {
            use std::os::windows::ffi::OsStringExt;
            let input = OsString::from_wide(&[0x68, 0xD800, 0x69]);
            let pe = PathElementCS::from_os_str(input).unwrap();
            assert_eq!(pe.original(), "h\u{FFFD}\u{FFFD}\u{FFFD}i");
        }

        // CI "H\tllo": original="H\tllo", os_compatible="H␉llo", normalized="h␉llo"
        // All three differ, so asserting "H\u{2409}llo" proves it's os_compatible.
        #[test]
        fn os_str_returns_os_compatible() {
            let pe = PathElementCI::new("H\tllo").unwrap();
            assert_eq!(pe.os_str(), OsStr::new("H\u{2409}llo"));
        }

        #[test]
        fn into_os_str_returns_os_compatible() {
            let pe = PathElementCI::new("H\tllo").unwrap();
            let result = pe.into_os_str();
            assert_eq!(result, OsStr::new("H\u{2409}llo"));
        }

        #[test]
        fn into_os_str_borrows_when_no_transformation() {
            let input = OsStr::new("hello.txt");
            let pe = PathElementCS::from_os_str(input).unwrap();
            let result = pe.into_os_str();
            assert!(matches!(result, Cow::Borrowed(_)));
            assert_eq!(result, OsStr::new("hello.txt"));
        }

        #[test]
        fn into_os_str_ci_borrows_when_already_folded() {
            let input = OsStr::new("hello.txt");
            let pe = PathElementCI::from_os_str(input).unwrap();
            let result = pe.into_os_str();
            assert!(matches!(result, Cow::Borrowed(_)));
            assert_eq!(result, OsStr::new("hello.txt"));
        }

        // Borrowed input, but NFC normalization produces owned output.
        // On Apple, the os-compatible form is NFD, which matches the NFD input, so it borrows.
        #[test]
        fn into_os_str_owned_when_nfc_transforms() {
            let input = OsStr::new("e\u{0301}.txt"); // NFD
            let pe = PathElementCS::from_os_str(input).unwrap();
            let result = pe.into_os_str();
            #[cfg(target_vendor = "apple")]
            assert!(matches!(result, Cow::Borrowed(_)));
            #[cfg(not(target_vendor = "apple"))]
            assert!(matches!(result, Cow::Owned(_)));
        }

        // NFC input borrows on non-Apple (os-compatible is NFC), owned on Apple (os-compatible is NFD).
        #[test]
        fn into_os_str_owned_when_nfd_transforms() {
            let input = OsStr::new("\u{00E9}.txt"); // NFC
            let pe = PathElementCS::from_os_str(input).unwrap();
            let result = pe.into_os_str();
            #[cfg(target_vendor = "apple")]
            assert!(matches!(result, Cow::Owned(_)));
            #[cfg(not(target_vendor = "apple"))]
            assert!(matches!(result, Cow::Borrowed(_)));
        }

        #[test]
        fn from_os_str_cs_matches_typed() {
            let input = OsStr::new("Hello.txt");
            let dyn_pe = PathElement::from_os_str_cs(input).unwrap();
            let cs_pe = PathElementCS::from_os_str(input).unwrap();
            assert_eq!(dyn_pe.normalized(), cs_pe.normalized());
            assert_eq!(dyn_pe.os_compatible(), cs_pe.os_compatible());
            assert_eq!(dyn_pe.case_sensitivity(), CaseSensitivity::Sensitive);
        }

        #[test]
        fn from_os_str_ci_matches_typed() {
            let input = OsStr::new("Hello.txt");
            let dyn_pe = PathElement::from_os_str_ci(input).unwrap();
            let ci_pe = PathElementCI::from_os_str(input).unwrap();
            assert_eq!(dyn_pe.normalized(), ci_pe.normalized());
            assert_eq!(dyn_pe.os_compatible(), ci_pe.os_compatible());
            assert_eq!(dyn_pe.case_sensitivity(), CaseSensitivity::Insensitive);
        }

        #[test]
        fn from_os_str_dynamic_case_sensitivity() {
            let input = OsStr::new("Hello.txt");
            let pe = PathElement::from_os_str(input, CaseInsensitive).unwrap();
            assert_eq!(pe.normalized(), "hello.txt");
        }
    }

    // --- from_bytes ---

    mod from_bytes_tests {
        use alloc::borrow::Cow;
        use alloc::vec;

        #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
        use wasm_bindgen_test::wasm_bindgen_test as test;

        use crate::case_sensitivity::{CaseInsensitive, CaseSensitive, CaseSensitivity};
        use crate::path_element::{PathElement, PathElementCI, PathElementCS};

        #[test]
        fn from_bytes_cs_borrowed_matches_new() {
            let pe_bytes = PathElementCS::from_bytes(b"hello.txt" as &[u8]).unwrap();
            let pe_str = PathElementCS::new("hello.txt").unwrap();
            assert_eq!(pe_bytes.original(), pe_str.original());
            assert_eq!(pe_bytes.normalized(), pe_str.normalized());
            assert_eq!(pe_bytes.os_compatible(), pe_str.os_compatible());
        }

        #[test]
        fn from_bytes_ci_borrowed_matches_new() {
            let pe_bytes = PathElementCI::from_bytes(b"Hello.txt" as &[u8]).unwrap();
            let pe_str = PathElementCI::new("Hello.txt").unwrap();
            assert_eq!(pe_bytes.original(), pe_str.original());
            assert_eq!(pe_bytes.normalized(), pe_str.normalized());
            assert_eq!(pe_bytes.os_compatible(), pe_str.os_compatible());
        }

        #[test]
        fn from_bytes_owned_matches_new() {
            let pe_bytes = PathElementCS::from_bytes(b"hello.txt".to_vec()).unwrap();
            let pe_str = PathElementCS::new("hello.txt").unwrap();
            assert_eq!(pe_bytes.original(), pe_str.original());
            assert_eq!(pe_bytes.normalized(), pe_str.normalized());
        }

        #[test]
        fn from_bytes_borrowed_preserves_borrow() {
            let input: &[u8] = b"hello.txt";
            let pe = PathElementCS::from_bytes(input).unwrap();
            let orig = pe.into_original();
            assert!(matches!(orig, Cow::Borrowed(_)));
        }

        #[test]
        fn from_bytes_owned_is_owned() {
            let pe = PathElementCS::from_bytes(b"hello.txt".to_vec()).unwrap();
            assert!(pe.is_owned());
        }

        #[test]
        fn from_bytes_invalid_utf8_borrowed_uses_replacement() {
            let input: &[u8] = &[0x68, 0x69, 0xFF]; // "hi" + invalid byte
            let pe = PathElementCS::from_bytes(input).unwrap();
            assert_eq!(pe.original(), "hi\u{FFFD}");
        }

        #[test]
        fn from_bytes_invalid_utf8_owned_uses_replacement() {
            let input = vec![0x68, 0x69, 0xFF];
            let pe = PathElementCS::from_bytes(input).unwrap();
            assert_eq!(pe.original(), "hi\u{FFFD}");
        }

        #[test]
        fn from_bytes_dynamic_case_sensitivity() {
            let pe = PathElement::from_bytes(b"Hello.txt" as &[u8], CaseInsensitive).unwrap();
            assert_eq!(pe.normalized(), "hello.txt");
            assert_eq!(pe.case_sensitivity(), CaseSensitivity::Insensitive);
        }

        #[test]
        fn from_bytes_cs_matches_typed() {
            let input: &[u8] = b"Hello.txt";
            let dyn_pe = PathElement::from_bytes_cs(input).unwrap();
            let cs_pe = PathElementCS::from_bytes(input).unwrap();
            assert_eq!(dyn_pe.normalized(), cs_pe.normalized());
            assert_eq!(dyn_pe.os_compatible(), cs_pe.os_compatible());
            assert_eq!(dyn_pe.case_sensitivity(), CaseSensitivity::Sensitive);
        }

        #[test]
        fn from_bytes_ci_matches_typed() {
            let input: &[u8] = b"Hello.txt";
            let dyn_pe = PathElement::from_bytes_ci(input).unwrap();
            let ci_pe = PathElementCI::from_bytes(input).unwrap();
            assert_eq!(dyn_pe.normalized(), ci_pe.normalized());
            assert_eq!(dyn_pe.os_compatible(), ci_pe.os_compatible());
            assert_eq!(dyn_pe.case_sensitivity(), CaseSensitivity::Insensitive);
        }

        #[test]
        fn from_bytes_with_case_sensitivity_cs() {
            let pe = PathElementCS::from_bytes(b"Hello.txt" as &[u8]).unwrap();
            assert_eq!(pe.normalized(), "Hello.txt");
        }

        #[test]
        fn from_bytes_with_case_sensitivity_ci() {
            let pe = PathElementCI::from_bytes(b"Hello.txt" as &[u8]).unwrap();
            assert_eq!(pe.normalized(), "hello.txt");
        }

        #[test]
        fn from_bytes_rejects_empty() {
            assert!(PathElementCS::from_bytes(b"" as &[u8]).is_err());
        }

        #[test]
        fn from_bytes_rejects_dot() {
            assert!(PathElementCS::from_bytes(b"." as &[u8]).is_err());
        }

        #[test]
        fn from_bytes_rejects_dotdot() {
            assert!(PathElementCS::from_bytes(b".." as &[u8]).is_err());
        }

        #[test]
        fn from_bytes_rejects_slash() {
            assert!(PathElementCS::from_bytes(b"a/b" as &[u8]).is_err());
        }

        #[test]
        fn from_bytes_rejects_null() {
            assert!(PathElementCS::from_bytes(b"\0" as &[u8]).is_err());
        }

        #[test]
        fn from_bytes_dynamic_sensitive() {
            let pe = PathElement::from_bytes(b"Hello.txt" as &[u8], CaseSensitive).unwrap();
            assert_eq!(pe.normalized(), "Hello.txt");
            assert_eq!(pe.case_sensitivity(), CaseSensitivity::Sensitive);
        }

        // --- Invalid byte decoding ---

        #[test]
        fn from_bytes_overlong_null() {
            // 0xC0 0x80 is an overlong encoding — replaced with U+FFFD per byte.
            let input: &[u8] = &[0x61, 0xC0, 0x80, 0x62]; // "a" + overlong + "b"
            let pe = PathElementCS::from_bytes(input).unwrap();
            assert_eq!(pe.original(), "a\u{FFFD}\u{FFFD}b");
        }

        #[test]
        fn from_bytes_surrogate_bytes_replaced() {
            // ED A0 BD ED B8 80 are surrogate half bytes — not valid UTF-8,
            // each invalid segment replaced with U+FFFD.
            let input: &[u8] = &[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80];
            let pe = PathElementCS::from_bytes(input).unwrap();
            assert!(pe.original().contains('\u{FFFD}'));
        }

        #[test]
        fn from_bytes_lone_high_surrogate_replaced() {
            let input: &[u8] = &[0x61, 0xED, 0xA0, 0x80, 0x62];
            let pe = PathElementCS::from_bytes(input).unwrap();
            assert_eq!(pe.original(), "a\u{FFFD}\u{FFFD}\u{FFFD}b");
        }

        #[test]
        fn from_bytes_lone_low_surrogate_replaced() {
            let input: &[u8] = &[0x61, 0xED, 0xB0, 0x80, 0x62];
            let pe = PathElementCS::from_bytes(input).unwrap();
            assert_eq!(pe.original(), "a\u{FFFD}\u{FFFD}\u{FFFD}b");
        }

        #[test]
        fn from_bytes_overlong_null_only() {
            let input: &[u8] = &[0xC0, 0x80];
            let pe = PathElementCS::from_bytes(input).unwrap();
            assert_eq!(pe.original(), "\u{FFFD}\u{FFFD}");
        }

        #[test]
        fn from_bytes_invalid_byte_replaced() {
            let input: &[u8] = &[0x68, 0x69, 0xFF]; // "hi" + invalid
            let pe = PathElementCS::from_bytes(input).unwrap();
            assert_eq!(pe.original(), "hi\u{FFFD}");
        }
    }

    // --- os_compatible supplementary character tests ---

    #[test]
    fn os_compatible_supplementary_unchanged() {
        let pe = PathElementCS::new("file_😀.txt").unwrap();
        assert_eq!(pe.os_compatible(), "file_😀.txt");
    }

    #[test]
    fn os_compatible_supplementary_roundtrip() {
        let pe = PathElementCS::new("file_😀.txt").unwrap();
        let pe2 = PathElementCS::new(pe.os_compatible()).unwrap();
        assert_eq!(pe.normalized(), pe2.normalized());
    }

    #[test]
    fn os_compatible_multiple_supplementary() {
        let pe = PathElementCS::new("𐀀_𝄞_😀").unwrap();
        assert_eq!(pe.os_compatible(), "𐀀_𝄞_😀");
    }
}
