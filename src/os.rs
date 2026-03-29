#[cfg(any(target_vendor = "apple", test, feature = "__test"))]
use alloc::vec::Vec;

#[cfg(target_vendor = "apple")]
use crate::Error;
use crate::Result;
#[cfg(any(target_os = "windows", test, feature = "__test"))]
use crate::unicode::case_fold;
#[cfg(all(not(target_vendor = "apple"), any(test, feature = "__test")))]
use crate::unicode::nfd;
#[cfg(any(target_vendor = "apple", test, feature = "__test"))]
use crate::utils::SubstringOrOwned;
#[cfg(any(target_os = "windows", test, feature = "__test"))]
use crate::utils::cow;
use crate::utils::str_cow_to_bytes;
use alloc::borrow::Cow;
#[cfg(any(target_os = "windows", test, feature = "__test"))]
use alloc::format;

/// Windows reserved device names (case-folded, checked against case-folded stem before first dot).
#[cfg(any(target_os = "windows", test, feature = "__test"))]
const WINDOWS_RESERVED: &[&str] = &[
    "con",
    "prn",
    "aux",
    "nul",
    "com0",
    "com1",
    "com2",
    "com3",
    "com4",
    "com5",
    "com6",
    "com7",
    "com8",
    "com9",
    "com\u{00B9}",
    "com\u{00B2}",
    "com\u{00B3}",
    "lpt0",
    "lpt1",
    "lpt2",
    "lpt3",
    "lpt4",
    "lpt5",
    "lpt6",
    "lpt7",
    "lpt8",
    "lpt9",
    "lpt\u{00B9}",
    "lpt\u{00B2}",
    "lpt\u{00B3}",
];

/// Windows forbidden characters mapped to their Fullwidth equivalents.
#[cfg(any(target_os = "windows", test, feature = "__test"))]
fn map_windows_forbidden(c: char) -> char {
    match c {
        '<' => '\u{FF1C}',
        '>' => '\u{FF1E}',
        ':' => '\u{FF1A}',
        '"' => '\u{FF02}',
        '\\' => '\u{FF3C}',
        '|' => '\u{FF5C}',
        '?' => '\u{FF1F}',
        '*' => '\u{FF0A}',
        _ => c,
    }
}

/// Whether the stem of `name` (the part before the first `.`) is a Windows reserved device name.
///
/// Comparison uses Unicode `toCasefold()` so that e.g. "CON", "con", and "Ｃon" (after
/// fullwidth mapping) all match.
#[cfg(any(target_os = "windows", test, feature = "__test"))]
pub fn is_reserved_on_windows(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    let folded = case_fold(stem);
    WINDOWS_RESERVED.iter().any(|r| **r == *folded)
}

/// Windows compatibility mapping: forbidden characters, trailing dots, and reserved names.
#[cfg(any(target_os = "windows", test, feature = "__test"))]
pub fn windows_compatible_from_normalized_cs(s: &str) -> Cow<'_, [u8]> {
    // Step 1: Map forbidden characters
    let mut result = cow(s.chars().map(map_windows_forbidden), s);

    // Step 2: Handle trailing dot
    if result.ends_with('.') {
        let owned = result.to_mut();
        owned.pop();
        owned.push('\u{FF0E}');
    }

    // Step 3: Handle reserved names
    if is_reserved_on_windows(&result) {
        let owned = result.into_owned();
        let first = owned.chars().next().expect("reserved name is non-empty");
        debug_assert!(
            first.is_ascii_alphabetic(),
            "reserved name starts with non-ASCII-letter: {first:?}"
        );
        let fullwidth = char::from_u32(first as u32 + 0xFEE0).unwrap_or(first);
        result = Cow::Owned(format!("{fullwidth}{}", &owned[first.len_utf8()..]));
    }

    str_cow_to_bytes(result)
}

/// Obtain the Darwin-native filesystem representation of a string via
/// `CFStringGetFileSystemRepresentation`.
#[cfg(target_vendor = "apple")]
fn apple_file_system_representation(s: &str) -> Result<Vec<u8>> {
    use objc2_core_foundation::CFString;

    let cf = CFString::from_str(s);
    let max_len = cf.maximum_size_of_file_system_representation();
    let mut buf = alloc::vec![0u8; max_len as usize];
    // Safety: buf is a valid, zero-initialized buffer of max_len bytes.
    // c_char and u8 have the same size; the cast is layout-compatible.
    let ok = unsafe { cf.file_system_representation(buf.as_mut_ptr().cast(), max_len) };
    if ok {
        let nul = buf.iter().position(|&b| b == 0).ok_or(Error::OSError)?;
        buf.truncate(nul);
        Ok(buf)
    } else {
        Err(Error::OSError)
    }
}

/// Portable fallback: NFD normalization + leading BOM removal.
#[cfg(all(not(target_vendor = "apple"), any(test, feature = "__test")))]
#[allow(clippy::unnecessary_wraps)]
fn apple_file_system_representation(s: &str) -> Result<Vec<u8>> {
    Ok(nfd(s).trim_start_matches('\u{FEFF}').as_bytes().to_vec())
}

/// Apple compatibility mapping: NFC to NFD conversion and BOM removal.
#[cfg(any(target_vendor = "apple", test, feature = "__test"))]
pub fn apple_compatible_from_normalized_cs(s: &str) -> Result<Cow<'_, [u8]>> {
    let bytes = apple_file_system_representation(s)?;
    let soo = SubstringOrOwned::new(&bytes[..], s.as_bytes());
    Ok(soo.into_cow(Cow::Borrowed(s.as_bytes())))
}

/// Apply the current OS's compatibility mapping.
#[cfg(target_os = "windows")]
#[allow(clippy::unnecessary_wraps)]
pub fn os_compatible_from_normalized_cs(s: &str) -> Result<Cow<'_, [u8]>> {
    Ok(windows_compatible_from_normalized_cs(s))
}

/// Apply the current OS's compatibility mapping.
#[cfg(target_vendor = "apple")]
pub fn os_compatible_from_normalized_cs(s: &str) -> Result<Cow<'_, [u8]>> {
    apple_compatible_from_normalized_cs(s)
}

/// Apply the current OS's compatibility mapping.
#[cfg(not(any(target_os = "windows", target_vendor = "apple")))]
#[allow(clippy::unnecessary_wraps)]
pub fn os_compatible_from_normalized_cs(s: &str) -> Result<Cow<'_, [u8]>> {
    Ok(crate::java_modified_utf8::str_to_os_bytes(Cow::Borrowed(s)))
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use alloc::format;

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{
        apple_compatible_from_normalized_cs, is_reserved_on_windows,
        os_compatible_from_normalized_cs, windows_compatible_from_normalized_cs,
    };
    use crate::unicode::{case_fold, nfd};

    // --- windows_compatible_from_normalized_cs ---

    #[test]
    fn win_forbidden_chars() {
        assert_eq!(
            windows_compatible_from_normalized_cs("a<b>c").as_ref(),
            "a\u{FF1C}b\u{FF1E}c".as_bytes()
        );
    }

    #[test]
    fn win_all_forbidden() {
        assert_eq!(
            windows_compatible_from_normalized_cs("<>:\"\\|?*").as_ref(),
            "\u{FF1C}\u{FF1E}\u{FF1A}\u{FF02}\u{FF3C}\u{FF5C}\u{FF1F}\u{FF0A}".as_bytes()
        );
    }

    #[test]
    fn win_trailing_dot() {
        assert_eq!(
            windows_compatible_from_normalized_cs("file.").as_ref(),
            "file\u{FF0E}".as_bytes()
        );
    }

    #[test]
    fn win_trailing_dots() {
        assert_eq!(
            windows_compatible_from_normalized_cs("file..").as_ref(),
            "file.\u{FF0E}".as_bytes()
        );
    }

    #[test]
    fn win_trailing_space_dot() {
        assert_eq!(
            windows_compatible_from_normalized_cs("file .").as_ref(),
            "file \u{FF0E}".as_bytes()
        );
    }

    #[test]
    fn win_reserved_presentation_nul() {
        assert_eq!(
            windows_compatible_from_normalized_cs("nul").as_ref(),
            "\u{FF4E}ul".as_bytes()
        );
    }

    #[test]
    fn win_reserved_presentation_with_ext() {
        assert_eq!(
            windows_compatible_from_normalized_cs("nul.txt").as_ref(),
            "\u{FF4E}ul.txt".as_bytes()
        );
    }

    #[test]
    fn win_reserved_presentation_com1() {
        assert_eq!(
            windows_compatible_from_normalized_cs("COM1").as_ref(),
            "\u{FF23}OM1".as_bytes()
        );
    }

    #[test]
    fn win_normal_unchanged() {
        let result = windows_compatible_from_normalized_cs("hello.txt");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), b"hello.txt");
    }

    // --- is_reserved_on_windows ---

    #[test]
    fn reserved_basic_names() {
        assert!(is_reserved_on_windows("con"));
        assert!(is_reserved_on_windows("prn"));
        assert!(is_reserved_on_windows("aux"));
        assert!(is_reserved_on_windows("nul"));
    }

    #[test]
    fn reserved_case_folded() {
        assert!(is_reserved_on_windows("CON"));
        assert!(is_reserved_on_windows("Con"));
        assert!(is_reserved_on_windows("NUL"));
        assert!(is_reserved_on_windows("Aux"));
    }

    #[test]
    fn reserved_com_digits() {
        for i in 0..=9 {
            assert!(is_reserved_on_windows(&format!("com{i}")));
            assert!(is_reserved_on_windows(&format!("COM{i}")));
            assert!(is_reserved_on_windows(&format!("lpt{i}")));
            assert!(is_reserved_on_windows(&format!("LPT{i}")));
        }
    }

    #[test]
    fn reserved_com_superscript() {
        // U+00B9 SUPERSCRIPT ONE, U+00B2 SUPERSCRIPT TWO, U+00B3 SUPERSCRIPT THREE
        assert!(is_reserved_on_windows("COM\u{00B9}"));
        assert!(is_reserved_on_windows("COM\u{00B2}"));
        assert!(is_reserved_on_windows("COM\u{00B3}"));
        assert!(is_reserved_on_windows("LPT\u{00B9}"));
        assert!(is_reserved_on_windows("LPT\u{00B2}"));
        assert!(is_reserved_on_windows("LPT\u{00B3}"));
    }

    #[test]
    fn reserved_with_extension() {
        assert!(is_reserved_on_windows("nul.txt"));
        assert!(is_reserved_on_windows("CON.log"));
        assert!(is_reserved_on_windows("COM1.dat"));
        assert!(is_reserved_on_windows("COM\u{00B3}.txt"));
    }

    #[test]
    fn not_reserved_longer_stem() {
        assert!(!is_reserved_on_windows("CONX"));
        assert!(!is_reserved_on_windows("nully"));
        assert!(!is_reserved_on_windows("com10"));
        assert!(!is_reserved_on_windows("lpt10"));
        assert!(!is_reserved_on_windows("auxiliary"));
    }

    #[test]
    fn not_reserved_stem_split_by_dot() {
        // "nu.l" → stem is "nu", not reserved
        assert!(!is_reserved_on_windows("nu.l"));
    }

    #[test]
    fn not_reserved_normal_files() {
        assert!(!is_reserved_on_windows("hello.txt"));
        assert!(!is_reserved_on_windows("readme"));
        assert!(!is_reserved_on_windows(".gitignore"));
    }

    #[test]
    fn reserved_stable_under_nfd() {
        // Reserved status should not change under NFD decomposition.
        for name in ["con", "nul", "COM1", "COM\u{00B9}", "LPT\u{00B3}"] {
            assert_eq!(
                is_reserved_on_windows(name),
                is_reserved_on_windows(&nfd(name)),
                "NFD changed reserved status for {name:?}"
            );
        }
    }

    #[test]
    fn reserved_stable_under_case_fold() {
        for name in ["CON", "Nul", "COM1", "LPT\u{00B2}"] {
            assert_eq!(
                is_reserved_on_windows(name),
                is_reserved_on_windows(&case_fold(name)),
                "case_fold changed reserved status for {name:?}"
            );
        }
    }

    // --- apple_compatible_from_normalized_cs ---

    #[test]
    fn apple_nfd_and_remove_bom() {
        assert_eq!(
            apple_compatible_from_normalized_cs("\u{FEFF}\u{00E9}")
                .unwrap()
                .as_ref(),
            "e\u{0301}".as_bytes()
        );
    }

    #[test]
    fn apple_ascii_unchanged() {
        let result = apple_compatible_from_normalized_cs("hello").unwrap();
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), b"hello");
    }

    #[test]
    fn apple_bom_removal_borrows() {
        let input = "\u{FEFF}hello";
        let result = apple_compatible_from_normalized_cs(input).unwrap();
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), b"hello");
        assert!(core::ptr::eq(
            result.as_ptr(),
            input["\u{FEFF}".len()..].as_ptr()
        ));
    }

    // --- os_compatible_from_normalized_cs ---

    #[test]
    fn os_compatible_from_normalized_cs_ascii_unchanged() {
        assert_eq!(
            os_compatible_from_normalized_cs("hello.txt")
                .unwrap()
                .as_ref(),
            b"hello.txt"
        );
    }

    #[test]
    fn os_compatible_from_normalized_cs_forbidden_chars() {
        let result = os_compatible_from_normalized_cs("a<b").unwrap();
        #[cfg(target_os = "windows")]
        assert_eq!(result.as_ref(), "a\u{FF1C}b".as_bytes());
        #[cfg(not(target_os = "windows"))]
        assert_eq!(result.as_ref(), b"a<b");
    }

    #[test]
    fn os_compatible_from_normalized_cs_reserved_name() {
        let result = os_compatible_from_normalized_cs("nul").unwrap();
        #[cfg(target_os = "windows")]
        assert_eq!(result.as_ref(), "\u{FF4E}ul".as_bytes());
        #[cfg(not(target_os = "windows"))]
        assert_eq!(result.as_ref(), b"nul");
    }

    #[test]
    fn os_compatible_from_normalized_cs_nfc_input() {
        let result = os_compatible_from_normalized_cs("\u{00E9}").unwrap();
        #[cfg(target_vendor = "apple")]
        assert_eq!(result.as_ref(), "e\u{0301}".as_bytes());
        #[cfg(not(target_vendor = "apple"))]
        assert_eq!(result.as_ref(), "\u{00E9}".as_bytes());
    }

    #[test]
    fn os_compatible_from_normalized_cs_bom() {
        let result = os_compatible_from_normalized_cs("\u{FEFF}hello").unwrap();
        #[cfg(target_vendor = "apple")]
        assert_eq!(result.as_ref(), b"hello");
        #[cfg(not(target_vendor = "apple"))]
        assert_eq!(result.as_ref(), "\u{FEFF}hello".as_bytes());
    }
}
