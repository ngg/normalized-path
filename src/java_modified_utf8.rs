use alloc::borrow::Cow;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::utils::{str_cow_to_bytes, to_java_modified_utf8};

static GLOBAL: AtomicBool = AtomicBool::new(false);

#[cfg(all(feature = "std", any(test, feature = "__test")))]
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
pub fn configure_java_modified_utf8(enabled: bool) {
    GLOBAL.store(enabled, Ordering::Relaxed);
}

/// Returns whether Java Modified UTF-8 encoding is enabled for OS-compatible output.
///
/// Returns `false` by default on all platforms.
/// See [`configure_java_modified_utf8()`] for details.
pub fn is_using_java_modified_utf8() -> bool {
    #[cfg(all(feature = "std", any(test, feature = "__test")))]
    {
        if let Some(v) = LOCAL.get() {
            return v;
        }
    }
    GLOBAL.load(Ordering::Relaxed)
}

/// Sets a thread-local override for the Java Modified UTF-8 flag.
///
/// Returns an RAII guard that restores the previous value when dropped.
/// This allows parallel tests to independently control the flag.
#[cfg(all(feature = "std", any(test, feature = "__test")))]
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

pub fn str_to_os_bytes(s: Cow<'_, str>) -> Cow<'_, [u8]> {
    if is_using_java_modified_utf8() {
        match to_java_modified_utf8(&s) {
            Cow::Borrowed(_) => str_cow_to_bytes(s),
            Cow::Owned(v) => Cow::Owned(v),
        }
    } else {
        str_cow_to_bytes(s)
    }
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{is_using_java_modified_utf8, str_to_os_bytes};
    use crate::utils::decode_utf8_lossy;

    #[test]
    fn str_to_os_bytes_passthrough_when_disabled() {
        assert!(!is_using_java_modified_utf8());
        let result = str_to_os_bytes(Cow::Borrowed("hello"));
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), b"hello");
    }

    #[test]
    fn str_to_os_bytes_roundtrip_when_disabled() {
        let input = "file_😀.txt";
        let os_bytes = str_to_os_bytes(Cow::Borrowed(input));
        let decoded = decode_utf8_lossy(&os_bytes);
        assert_eq!(&*decoded, input);
    }

    #[test]
    fn str_to_os_bytes_supplementary_without_flag() {
        let result = str_to_os_bytes(Cow::Borrowed("😀"));
        assert_eq!(result.as_ref(), "😀".as_bytes());
    }

    #[cfg(feature = "std")]
    mod std_tests {
        use alloc::borrow::Cow;

        #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
        use wasm_bindgen_test::wasm_bindgen_test as test;

        use super::super::{
            is_using_java_modified_utf8, str_to_os_bytes, thread_override_java_modified_utf8,
        };
        use crate::path_element::PathElementCS;
        use crate::utils::decode_utf8_lossy;

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
        fn str_to_os_bytes_supplementary_with_flag() {
            let _guard = thread_override_java_modified_utf8(true);
            let result = str_to_os_bytes(Cow::Borrowed("😀"));
            assert_eq!(result.as_ref(), &[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        }

        #[test]
        fn str_to_os_bytes_ascii_unchanged_with_flag() {
            let _guard = thread_override_java_modified_utf8(true);
            let result = str_to_os_bytes(Cow::Borrowed("hello.txt"));
            assert_eq!(result.as_ref(), b"hello.txt");
        }

        #[test]
        fn str_to_os_bytes_bmp_unchanged_with_flag() {
            let _guard = thread_override_java_modified_utf8(true);
            let result = str_to_os_bytes(Cow::Borrowed("café"));
            assert_eq!(result.as_ref(), "café".as_bytes());
        }

        #[test]
        fn str_to_os_bytes_roundtrip_with_flag() {
            let _guard = thread_override_java_modified_utf8(true);
            let input = "file_😀.txt";
            let os_bytes = str_to_os_bytes(Cow::Borrowed(input));
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
        fn str_to_os_bytes_borrows_when_no_supplementary() {
            let _guard = thread_override_java_modified_utf8(true);
            let result = str_to_os_bytes(Cow::Borrowed("hello"));
            assert!(matches!(result, Cow::Borrowed(_)));
        }
    }
}
