use alloc::borrow::Cow;
use alloc::string::String;

use crate::utils::cow_str_to_bytes;

#[cfg(all(unix, not(target_vendor = "apple")))]
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(all(unix, not(target_vendor = "apple")))]
static GLOBAL: AtomicBool = AtomicBool::new(false);

#[cfg(all(
    unix,
    not(target_vendor = "apple"),
    feature = "std",
    any(test, feature = "__test")
))]
std::thread_local! {
    static LOCAL: core::cell::Cell<Option<bool>> = const { core::cell::Cell::new(None) };
}

/// Enables or disables Java Modified UTF-8 encoding for OS-compatible output.
///
/// When enabled, [`PathElementGeneric::os_compatible()`](crate::PathElementGeneric::os_compatible)
/// returns bytes encoded in Java's Modified UTF-8 format, where supplementary
/// characters (U+10000 and above) are represented as CESU-8 surrogate pairs.
///
/// The default is `false` on all platforms. This should be called once at
/// application startup, before constructing any path elements, on Android
/// runtimes that use Java Modified UTF-8 for filesystem paths.
#[cfg(all(unix, not(target_vendor = "apple")))]
pub fn configure_java_modified_utf8(enabled: bool) {
    GLOBAL.store(enabled, Ordering::Relaxed);
}

/// Returns whether Java Modified UTF-8 encoding is enabled for OS-compatible output.
///
/// Returns `false` by default on all platforms. On platforms where
/// [`configure_java_modified_utf8()`] is not available, always returns `false`.
#[must_use]
pub fn is_using_java_modified_utf8() -> bool {
    #[cfg(all(unix, not(target_vendor = "apple")))]
    {
        #[cfg(all(feature = "std", any(test, feature = "__test")))]
        if let Some(v) = LOCAL.get() {
            return v;
        }
        return GLOBAL.load(Ordering::Relaxed);
    }
    #[allow(unreachable_code)]
    false
}

/// Sets a thread-local override for the Java Modified UTF-8 flag.
///
/// Returns an RAII guard that restores the previous value when dropped.
/// This allows parallel tests to independently control the flag.
#[cfg(all(
    unix,
    not(target_vendor = "apple"),
    feature = "std",
    any(test, feature = "__test")
))]
#[must_use]
pub fn thread_override_java_modified_utf8(val: bool) -> impl Drop {
    struct Guard(Option<bool>);
    impl Drop for Guard {
        fn drop(&mut self) {
            LOCAL.set(self.0);
        }
    }
    let prev = LOCAL.get();
    LOCAL.set(Some(val));
    Guard(prev)
}

#[cfg_attr(any(target_os = "windows", target_vendor = "apple"), allow(dead_code))]
#[must_use]
pub fn encode_java_modified_utf8(s: &str) -> Cow<'_, [u8]> {
    simd_cesu8::encode(s)
}

#[must_use]
pub fn decode_utf8_lossy(bytes: &[u8]) -> Cow<'_, str> {
    simd_cesu8::decode(bytes).unwrap_or_else(|_| String::from_utf8_lossy(bytes))
}

#[cfg_attr(any(target_os = "windows", target_vendor = "apple"), allow(dead_code))]
pub fn encode_os_utf8(s: Cow<'_, str>) -> Cow<'_, [u8]> {
    if is_using_java_modified_utf8() {
        match encode_java_modified_utf8(&s) {
            Cow::Borrowed(_) => cow_str_to_bytes(s),
            Cow::Owned(v) => Cow::Owned(v),
        }
    } else {
        cow_str_to_bytes(s)
    }
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use alloc::vec;

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{decode_utf8_lossy, encode_java_modified_utf8, is_using_java_modified_utf8};

    #[test]
    fn default_is_false() {
        assert!(!is_using_java_modified_utf8());
    }

    // --- decode_utf8_lossy ---

    #[test]
    fn decode_lossy_valid_utf8_borrows() {
        let result = decode_utf8_lossy(b"hello");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn decode_lossy_empty_borrows() {
        let result = decode_utf8_lossy(b"");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "");
    }

    #[test]
    fn decode_lossy_valid_multibyte_borrows() {
        let result = decode_utf8_lossy("é日本語".as_bytes());
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "é日本語");
    }

    #[test]
    fn decode_lossy_valid_4byte_borrows() {
        let result = decode_utf8_lossy("😀".as_bytes());
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "😀");
    }

    #[test]
    fn decode_lossy_overlong_null() {
        let result = decode_utf8_lossy(&[0xC0, 0x80]);
        assert_eq!(result, "\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn decode_lossy_overlong_null_in_context() {
        let result = decode_utf8_lossy(&[0x61, 0xC0, 0x80, 0x62]);
        assert_eq!(result, "a\u{FFFD}\u{FFFD}b");
    }

    #[test]
    fn decode_lossy_cesu8_surrogate_pair() {
        let result = decode_utf8_lossy(&[0xED, 0xA0, 0x80, 0xED, 0xB0, 0x80]);
        assert_eq!(result, "\u{10000}");
    }

    #[test]
    fn decode_lossy_cesu8_emoji() {
        let result = decode_utf8_lossy(&[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        assert_eq!(result, "😀");
    }

    #[test]
    fn decode_lossy_cesu8_in_context() {
        let result = decode_utf8_lossy(&[0x68, 0x69, 0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80, 0x21]);
        assert_eq!(result, "hi😀!");
    }

    #[test]
    fn decode_lossy_lone_high_surrogate() {
        let result = decode_utf8_lossy(&[0xED, 0xA0, 0x80, 0x61]);
        assert_eq!(result, "\u{FFFD}\u{FFFD}\u{FFFD}a");
    }

    #[test]
    fn decode_lossy_lone_low_surrogate() {
        let result = decode_utf8_lossy(&[0xED, 0xB0, 0x80]);
        assert_eq!(result, "\u{FFFD}\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn decode_lossy_high_surrogate_followed_by_non_surrogate() {
        let result = decode_utf8_lossy(&[0xED, 0xA0, 0x80, 0xE3, 0x81, 0x82]);
        assert_eq!(result, "\u{FFFD}\u{FFFD}\u{FFFD}あ");
    }

    #[test]
    fn decode_lossy_invalid_byte_replaced() {
        let result = decode_utf8_lossy(&[0x68, 0x69, 0xFF]);
        assert_eq!(result, "hi\u{FFFD}");
    }

    #[test]
    fn decode_lossy_invalid_continuation() {
        let result = decode_utf8_lossy(&[0xC3, 0x00]);
        assert_eq!(result, "\u{FFFD}\0");
    }

    #[test]
    fn decode_lossy_overlong_2byte_rejected() {
        let result = decode_utf8_lossy(&[0xC0, 0xBF]);
        assert_eq!(result, "\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn decode_lossy_overlong_3byte_rejected() {
        let result = decode_utf8_lossy(&[0xE0, 0x81, 0xBF]);
        assert_eq!(result, "\u{FFFD}\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn decode_lossy_truncated_2byte() {
        let result = decode_utf8_lossy(&[0x61, 0xC3]);
        assert_eq!(result, "a\u{FFFD}");
    }

    #[test]
    fn decode_lossy_truncated_3byte() {
        let result = decode_utf8_lossy(&[0x61, 0xE3, 0x81]);
        assert_eq!(result, "a\u{FFFD}");
    }

    #[test]
    fn decode_lossy_truncated_4byte() {
        let result = decode_utf8_lossy(&[0x61, 0xF0, 0x9F, 0x98]);
        assert_eq!(result, "a\u{FFFD}");
    }

    #[test]
    fn decode_lossy_mixed_invalid_with_cesu8() {
        let mut input = vec![0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80];
        input.push(0xFF);
        let result = decode_utf8_lossy(&input);
        assert_eq!(
            result,
            "\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}"
        );
    }

    #[test]
    fn decode_lossy_pure_cesu8() {
        let mut input = vec![0x61];
        input.extend_from_slice(&[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        input.push(0x62);
        let result = decode_utf8_lossy(&input);
        assert_eq!(result, "a😀b");
    }

    #[test]
    fn decode_lossy_utf8_emoji_plus_invalid() {
        let mut input = "😀".as_bytes().to_vec();
        input.push(0xFF);
        let result = decode_utf8_lossy(&input);
        assert_eq!(result, "😀\u{FFFD}");
    }

    // --- encode_java_modified_utf8 ---

    #[test]
    fn cesu8_ascii_borrows() {
        let result = encode_java_modified_utf8("hello");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), b"hello");
    }

    #[test]
    fn cesu8_empty_borrows() {
        let result = encode_java_modified_utf8("");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn cesu8_bmp_borrows() {
        let result = encode_java_modified_utf8("éàü");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), "éàü".as_bytes());
    }

    #[test]
    fn cesu8_null_passthrough() {
        let result = encode_java_modified_utf8("\0");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), &[0x00]);
    }

    #[test]
    fn cesu8_null_in_context() {
        let result = encode_java_modified_utf8("a\0b");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), &[0x61, 0x00, 0x62]);
    }

    #[test]
    fn cesu8_supplementary_as_surrogate_pair() {
        let result = encode_java_modified_utf8("😀");
        assert_eq!(result.as_ref(), &[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
    }

    #[test]
    fn cesu8_supplementary_in_context() {
        let result = encode_java_modified_utf8("hi😀!");
        let mut expected = vec![0x68, 0x69];
        expected.extend_from_slice(&[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        expected.push(0x21);
        assert_eq!(result.as_ref(), &expected);
    }

    #[test]
    fn cesu8_u10000() {
        let result = encode_java_modified_utf8("\u{10000}");
        assert_eq!(result.as_ref(), &[0xED, 0xA0, 0x80, 0xED, 0xB0, 0x80]);
    }

    #[test]
    fn cesu8_roundtrip_with_decode() {
        for s in &["hello", "a\0b", "😀", "\u{10000}", "hi😀\0world"] {
            let encoded = encode_java_modified_utf8(s);
            let decoded = decode_utf8_lossy(&encoded);
            assert_eq!(&*decoded, *s, "roundtrip failed for {s:?}");
        }
    }

    #[cfg(all(unix, not(target_vendor = "apple")))]
    mod unix_tests {
        use alloc::borrow::Cow;

        use super::super::{encode_os_utf8, is_using_java_modified_utf8};
        use super::decode_utf8_lossy;

        #[test]
        fn encode_os_utf8_passthrough_when_disabled() {
            assert!(!is_using_java_modified_utf8());
            let result = encode_os_utf8(Cow::Borrowed("hello"));
            assert!(matches!(result, Cow::Borrowed(_)));
            assert_eq!(result.as_ref(), b"hello");
        }

        #[test]
        fn encode_os_utf8_roundtrip_when_disabled() {
            let input = "file_😀.txt";
            let os_bytes = encode_os_utf8(Cow::Borrowed(input));
            let decoded = decode_utf8_lossy(&os_bytes);
            assert_eq!(&*decoded, input);
        }

        #[test]
        fn encode_os_utf8_supplementary_without_flag() {
            let result = encode_os_utf8(Cow::Borrowed("😀"));
            assert_eq!(result.as_ref(), "😀".as_bytes());
        }
    }

    #[cfg(all(unix, not(target_vendor = "apple"), feature = "std"))]
    mod std_tests {
        use alloc::borrow::Cow;

        use super::super::{
            encode_os_utf8, is_using_java_modified_utf8, thread_override_java_modified_utf8,
        };
        use super::decode_utf8_lossy;
        use crate::path_element::PathElementCS;

        #[test]
        fn flag_override_false() {
            let _guard = thread_override_java_modified_utf8(false);
            assert!(!is_using_java_modified_utf8());
        }

        #[test]
        fn flag_override_true() {
            let _guard = thread_override_java_modified_utf8(true);
            assert!(is_using_java_modified_utf8());
        }

        #[test]
        fn flag_override_restores_on_drop() {
            {
                let _guard = thread_override_java_modified_utf8(true);
                assert!(is_using_java_modified_utf8());
            }
            assert!(!is_using_java_modified_utf8());
        }

        #[test]
        fn flag_override_nested() {
            let _outer = thread_override_java_modified_utf8(true);
            assert!(is_using_java_modified_utf8());
            {
                let _inner = thread_override_java_modified_utf8(false);
                assert!(!is_using_java_modified_utf8());
            }
            assert!(is_using_java_modified_utf8());
        }

        #[test]
        fn encode_os_utf8_supplementary_with_flag() {
            let _guard = thread_override_java_modified_utf8(true);
            let result = encode_os_utf8(Cow::Borrowed("😀"));
            assert_eq!(result.as_ref(), &[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        }

        #[test]
        fn encode_os_utf8_ascii_unchanged_with_flag() {
            let _guard = thread_override_java_modified_utf8(true);
            let result = encode_os_utf8(Cow::Borrowed("hello.txt"));
            assert_eq!(result.as_ref(), b"hello.txt");
        }

        #[test]
        fn encode_os_utf8_bmp_unchanged_with_flag() {
            let _guard = thread_override_java_modified_utf8(true);
            let result = encode_os_utf8(Cow::Borrowed("café"));
            assert_eq!(result.as_ref(), "café".as_bytes());
        }

        #[test]
        fn encode_os_utf8_roundtrip_with_flag() {
            let _guard = thread_override_java_modified_utf8(true);
            let input = "file_😀.txt";
            let os_bytes = encode_os_utf8(Cow::Borrowed(input));
            let decoded = decode_utf8_lossy(&os_bytes);
            assert_eq!(&*decoded, input);
        }

        #[test]
        fn path_element_os_compatible_uses_modified_utf8() {
            let _guard = thread_override_java_modified_utf8(true);
            let pe = PathElementCS::new("file_😀.txt").unwrap();
            let os_bytes = pe.os_compatible();
            // 😀 is U+1F600, encoded as CESU-8 surrogate pair: 6 bytes instead of 4
            assert_ne!(os_bytes, "file_😀.txt".as_bytes());
            assert_eq!(os_bytes.len(), "file_😀.txt".len() + 2);
            // Round-trip still works
            let pe2 = PathElementCS::from_bytes(os_bytes).unwrap();
            assert_eq!(pe.normalized(), pe2.normalized());
        }

        #[test]
        fn encode_os_utf8_borrows_when_no_supplementary() {
            let _guard = thread_override_java_modified_utf8(true);
            let result = encode_os_utf8(Cow::Borrowed("hello"));
            assert!(matches!(result, Cow::Borrowed(_)));
        }
    }
}
