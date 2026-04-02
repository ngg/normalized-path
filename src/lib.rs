//! Opinionated cross-platform, optionally case-insensitive path normalization.
//!
//! This crate provides [`PathElementCS`] (case-sensitive), [`PathElementCI`]
//! (case-insensitive), and [`PathElement`] (runtime-selected) -- types that take a
//! raw path element name, validate it, normalize it to a canonical form, and compute
//! an OS-compatible presentation form.
//!
//! # Design goals and non-goals
//!
//! **Goals:**
//!
//! - The normalization procedure is identical on every platform -- the same input
//!   always produces the same normalized bytes regardless of the host OS.
//! - If any supported OS considers two names equivalent (e.g. NFC vs NFD on macOS),
//!   they must normalize to the same value.
//! - The normalized form is always in NFC (Unicode Normalization Form C), the
//!   most widely used and compact canonical form.
//! - Normalization is idempotent: normalizing an already-normalized name always
//!   produces the same name unchanged.
//! - The OS-compatible form of a name, when normalized again, produces the same
//!   normalized value as the original input (round-trip stability).
//! - Every valid name is representable on every supported OS.  Characters that
//!   would be rejected or silently altered (Windows forbidden characters, C0 controls)
//!   are mapped to visually similar safe alternatives.
//! - If the OS automatically transforms a name (e.g. NFC↔NFD conversion,
//!   truncation at null bytes), normalizing the transformed name produces the
//!   same result as normalizing the original.
//! - In case-insensitive mode, names differing only in case normalize identically,
//!   with correct handling of edge cases like Turkish dotted/dotless I.
//!
//! **Non-goals:**
//!
//! - Not every name that a particular OS accepts is considered valid.  Non-UTF-8
//!   byte sequences, names that normalize to empty (e.g. whitespace-only), and
//!   names that normalize to `.` or `..` (e.g. `" .. "`) are always rejected.
//! - A name taken directly from the OS may produce a different OS-compatible form
//!   after normalization.  For example, a file named `" hello.txt"` (leading space)
//!   will have the space trimmed, so its OS-compatible form is `"hello.txt"`.
//! - The OS-compatible form is not guaranteed to be accepted by the OS.  For
//!   example, it may exceed the OS's path element length limit, or on Apple
//!   platforms the filesystem may require names in Unicode Stream-Safe Text Format
//!   which the OS-compatible form does not enforce.
//! - Windows 8.3 short file names (e.g. `PROGRA~1`) are not handled.
//! - Visually similar names are not necessarily considered equal.  For example,
//!   a regular space (U+0020) and a non-breaking space (U+00A0) produce different
//!   normalized forms despite looking identical.
//! - Fullwidth and ASCII variants of the same character (e.g. `Ａ` vs `A`) are
//!   deliberately normalized to the same form.  Users who need to distinguish
//!   them cannot use this crate.
//! - Path separators and multi-component paths are not handled.  This crate
//!   operates on a single path element (one name between separators).  Support
//!   for full paths may be added in a future version.
//! - Android versions before 6 (API level 23) are not supported.  Earlier
//!   versions used Java Modified UTF-8 for filesystem paths, encoding
//!   supplementary characters as CESU-8 surrogate pairs.
//!
//! # Normalization pipeline
//!
//! Every path element name goes through the following steps during construction:
//!
//! 0. **Byte decoding** (only for `from_bytes`/`from_os_str`) --
//!    [`String::from_utf8_lossy()`] is applied, replacing invalid byte sequences
//!    with U+FFFD.  Invalid bytes can be encountered on Unix filesystems, which
//!    allow arbitrary bytes except `/` and `\0` in names, and on Windows, where
//!    filenames are WTF-16 and may contain unpaired surrogates.
//!
//! 1. **NFD decomposition** -- canonical decomposition to reorder combining marks.
//!    This is needed because macOS stores filenames in a form close to NFD, so an
//!    NFD input and an NFC input must produce the same result.  Decomposing first
//!    ensures combining marks are in canonical order before subsequent steps.
//!
//! 2. **Whitespace trimming** -- strips leading and trailing characters with the Unicode
//!    `White_Space` property, plus the BOM (U+FEFF) and Control Pictures that correspond
//!    to whitespace control characters (U+2409--U+240D: HT, LF, VT, FF, CR).
//!    Many applications strip leading/trailing whitespace silently, and macOS
//!    automatically strips leading BOMs.  Control Pictures are
//!    included because they are the mapped form of whitespace control characters
//!    (see step 4), so trimming must be consistent before and after mapping.
//!
//! 3. **Fullwidth-to-ASCII mapping** -- maps fullwidth forms (U+FF01--U+FF5E) to their
//!    ASCII equivalents (U+0021--U+007E).  The Windows OS-compatibility step (see below)
//!    maps certain ASCII characters to fullwidth to avoid Windows restrictions.  This
//!    step ensures that the OS-compatible form normalizes back to the same value.
//!
//! 4. **Control character mapping** -- maps C0 controls (U+0001--U+001F) and DEL (U+007F)
//!    to their Unicode Control Picture equivalents (U+2401--U+241F, U+2421).  Control
//!    characters are invisible, can break terminals and tools, and some OSes reject
//!    or silently drop them.  Mapping to visible Control Pictures preserves the
//!    information while making the name safe.  (Null bytes are excluded — see step 5.)
//!
//! 5. **Validation** -- rejects empty strings, `.`, `..`, names containing `/`, and
//!    names containing null bytes (`\0`).  These are universally special on all OSes
//!    and cannot be used as regular names.
//!
//! 6. **NFC composition** -- canonical composition to produce the shortest equivalent
//!    form.
//!
//! In **case-insensitive** mode, three additional steps are applied after the above:
//!
//! 7. **NFD decomposition** (again, on the NFC result).  Steps 7--8--10 implement
//!    the Unicode canonical caseless matching algorithm (Definition D145): *"A string
//!    X is a canonical caseless match for a string Y if and only if:
//!    NFD(toCasefold(NFD(X))) = NFD(toCasefold(NFD(Y)))"*, with an additional
//!    Turkish I/i fixup in step 9.
//!
//! 8. **Unicode `toCasefold()`** -- locale-independent case folding.
//!
//! 9. **Turkish I/i mapping** -- maps U+0130 (İ) and U+0131 (ı) to ASCII I and i
//!    respectively, and strips U+0307 COMBINING DOT ABOVE after I/i (with intervening
//!    non-starter combiners allowed).  Unicode `toCasefold()` is locale-independent
//!    and treats ı as distinct from i (ı folds to itself), yet `toUppercase(ı)` = I
//!    even without locale tailoring, and I folds back to i -- creating a collision
//!    that `toCasefold()` alone misses.
//!    This post-folding fixup neutralizes those inconsistencies.
//!
//! 10. **NFC composition** (final) -- recompose after case folding to produce the
//!     canonical NFC output.
//!
//! # OS compatibility mapping
//!
//! Each `PathElementGeneric` also computes an **OS-compatible** form suitable for
//! use as an actual path element name on the host operating system. It is derived
//! from the case-sensitive normalized form, by applying the following additional
//! steps:
//!
//! - **Windows**: the characters and patterns listed in the Windows
//!   [naming conventions](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file#naming-conventions)
//!   are handled by mapping them to visually similar fullwidth Unicode equivalents:
//!   forbidden characters (`< > : " \ | ? *`), the final trailing dot, and the first
//!   character of reserved device names (CON, PRN, AUX, NUL, COM0--COM9, LPT0--LPT9,
//!   and their superscript-digit variants).
//! - **Apple (macOS/iOS)**: converted using [`CFStringGetFileSystemRepresentation`](https://developer.apple.com/documentation/corefoundation/cfstringgetfilesystemrepresentation(_:_:_:))
//!   as recommended by Apple's documentation (produces a representation similar to NFD).
//! - **Other platforms**: the OS-compatible form is identical to the case-sensitive
//!   normalized form.
//!
//! # Types
//!
//! The core type is [`PathElementGeneric<'a, S>`], parameterized by a case-sensitivity
//! marker `S`:
//!
//! - [`PathElementCS`] = `PathElementGeneric<'a, CaseSensitive>` -- compile-time
//!   case-sensitive path element.
//! - [`PathElementCI`] = `PathElementGeneric<'a, CaseInsensitive>` -- compile-time
//!   case-insensitive path element.
//! - [`PathElement`] = `PathElementGeneric<'a, CaseSensitivity>` -- runtime-selected
//!   case sensitivity via the [`CaseSensitivity`] enum.
//!
//! Use the typed aliases ([`PathElementCS`], [`PathElementCI`]) when the case sensitivity
//! is known at compile time. These implement [`Hash`](core::hash::Hash), which the
//! runtime-dynamic [`PathElement`] does not (since hashing elements with different
//! sensitivities into the same map would violate hash/eq consistency).
//!
//! The zero-sized marker structs [`CaseSensitive`] and [`CaseInsensitive`] are used as
//! type parameters, while the [`CaseSensitivity`] enum provides the same choice at runtime.
//! All three types implement `Into<CaseSensitivity>`.
//!
//! # Examples
//!
//! ```
//! # use normalized_path::{PathElementCS, PathElementCI};
//! // NFD input (e + combining acute) composes to NFC (é), whitespace is trimmed
//! let pe = PathElementCS::new("  cafe\u{0301}.txt  ")?;
//! assert_eq!(pe.original(), "  cafe\u{0301}.txt  ");
//! assert_eq!(pe.normalized(), "caf\u{00E9}.txt");
//!
//! // Case-insensitive: German ß case-folds to "ss"
//! let pe = PathElementCI::new("Stra\u{00DF}e.txt")?;
//! assert_eq!(pe.original(), "Stra\u{00DF}e.txt");
//! assert_eq!(pe.normalized(), "strasse.txt");
//! # Ok::<(), normalized_path::Error>(())
//! ```
//!
//! The OS-compatible form adapts names for the host filesystem.  On Windows,
//! forbidden characters and reserved device names are mapped to safe alternatives;
//! on Apple, names are converted to a form close to NFD:
//!
//! ```
//! # use normalized_path::PathElementCS;
//! // A name with a Windows-forbidden character and an accented letter
//! let pe = PathElementCS::new("caf\u{00E9} 10:30")?;
//! assert_eq!(pe.normalized(), "caf\u{00E9} 10:30");
//!
//! #[cfg(target_os = "windows")]
//! assert_eq!(pe.os_compatible(), "caf\u{00E9} 10\u{FF1A}30"); // : → fullwidth ：
//!
//! #[cfg(target_vendor = "apple")]
//! assert_eq!(pe.os_compatible(), "cafe\u{0301} 10:30"); // NFC → NFD
//!
//! #[cfg(not(any(target_os = "windows", target_vendor = "apple")))]
//! assert_eq!(pe.os_compatible(), pe.normalized()); // unchanged
//! # Ok::<(), normalized_path::Error>(())
//! ```
//!
//! Equality is based on the normalized form, so different originals can compare equal:
//!
//! ```
//! # use normalized_path::PathElementCS;
//! // NFD (e + combining acute) and NFC (é) normalize to the same form
//! let a = PathElementCS::new("cafe\u{0301}.txt")?;
//! let b = PathElementCS::new("caf\u{00E9}.txt")?;
//! assert_eq!(a, b);
//! assert_ne!(a.original(), b.original());
//! # Ok::<(), normalized_path::Error>(())
//! ```
//!
//! The typed variants implement [`Hash`](core::hash::Hash), so they work in
//! both hash-based and ordered collections:
//!
//! ```
//! # use std::collections::{BTreeSet, HashSet};
//! # use normalized_path::PathElementCI;
//! // Turkish İ, dotless ı, ASCII I, and ASCII i all normalize to the same CI form
//! let names = ["\u{0130}.txt", "\u{0131}.txt", "I.txt", "i.txt"];
//! let set: HashSet<_> = names.iter().map(|n| PathElementCI::new(*n).unwrap()).collect();
//! assert_eq!(set.len(), 1);
//!
//! let tree: BTreeSet<_> = names.iter().map(|n| PathElementCI::new(*n).unwrap()).collect();
//! assert_eq!(tree.len(), 1);
//! ```
//!
//! The runtime-dynamic [`PathElement`] works in ordered collections too, but
//! comparing or ordering elements with **different** case sensitivities will panic:
//!
//! ```
//! # use std::collections::BTreeSet;
//! # use normalized_path::{PathElement, CaseSensitive};
//! let names = ["README.md", "readme.md", "Readme.MD"];
//! let tree: BTreeSet<_> = names.iter()
//!     .map(|n| PathElement::new(*n, CaseSensitive).unwrap())
//!     .collect();
//! assert_eq!(tree.len(), 3); // case-sensitive: all distinct
//! ```
//!
//! # Unicode version
//!
//! All Unicode operations (NFC, NFD, case folding, property lookups) use
//! **Unicode 17.0.0**. The Unicode version is considered part of the crate's
//! stability contract: it will only be updated in a semver-breaking release to
//! ensure that normalization results are consistent across all compatible versions.
//!
//! # `no_std` support
//!
//! This crate supports `no_std` environments. Disable the default `std` feature:
//!
//! ```toml
//! [dependencies]
//! normalized-path = { version = "...", default-features = false }
//! ```
//!
//! The `std` feature enables `from_os_str` constructors and
//! `os_str`/`into_os_str` accessors. The `alloc` crate is always required.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(clippy::all, clippy::pedantic)]

extern crate alloc;

mod case_sensitivity;
mod error;
mod normalize;
mod os;
mod path_element;
mod unicode;
mod utils;

pub use case_sensitivity::{CaseInsensitive, CaseSensitive, CaseSensitivity};
pub use error::{Error, ErrorKind, Result};
pub use path_element::{PathElement, PathElementCI, PathElementCS, PathElementGeneric};

#[cfg(any(feature = "__test", test))]
pub mod test_helpers {
    pub use crate::error::ResultKind;
    pub use crate::normalize::{
        is_whitespace_like, map_control_chars, map_fullwidth, map_turkish_i,
        normalize_ci_from_normalized_cs, normalize_cs, trim_whitespace_like, validate_path_element,
    };
    pub use crate::os::{
        apple_compatible_from_normalized_cs, apple_compatible_from_normalized_cs_fallback,
        is_reserved_on_windows, windows_compatible_from_normalized_cs,
    };
    pub use crate::unicode::{case_fold, is_starter, is_whitespace, nfc, nfd};
}
