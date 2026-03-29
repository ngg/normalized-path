use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use alloc::string::String;
use core::borrow::Borrow;
use core::ops::{Index, Range};

#[cfg(any(
    not(any(target_os = "windows", target_vendor = "apple")),
    test,
    feature = "__test"
))]
#[must_use]
pub fn to_java_modified_utf8(s: &str) -> Cow<'_, [u8]> {
    simd_cesu8::encode(s)
}

#[must_use]
pub fn decode_utf8_lossy(bytes: &[u8]) -> Cow<'_, str> {
    simd_cesu8::decode(bytes).unwrap_or_else(|_| String::from_utf8_lossy(bytes))
}

/// Trait abstracting over `str` and `[u8]` for [`SubstringOrOwned`].
pub trait Segment: ToOwned + Index<Range<usize>, Output = Self> {
    fn as_byte_slice(&self) -> &[u8];
    fn find_subslice(&self, needle: &Self) -> Option<usize>;
}

impl Segment for str {
    fn as_byte_slice(&self) -> &[u8] {
        self.as_bytes()
    }

    fn find_subslice(&self, needle: &str) -> Option<usize> {
        self.find(needle)
    }
}

impl Segment for [u8] {
    fn as_byte_slice(&self) -> &[u8] {
        self
    }

    fn find_subslice(&self, needle: &[u8]) -> Option<usize> {
        memchr::memmem::find(self, needle)
    }
}

/// Convert a `Cow<str>` to `Cow<[u8]>` without copying when possible.
pub fn str_cow_to_bytes(cow: Cow<'_, str>) -> Cow<'_, [u8]> {
    match cow {
        Cow::Borrowed(s) => Cow::Borrowed(s.as_bytes()),
        Cow::Owned(s) => Cow::Owned(s.into_bytes()),
    }
}

/// Compare `original` char-by-char against `converted`; return `Cow::Borrowed` when:
/// - All characters match (returns full `original`),
/// - `converted` is a prefix of `original` (returns borrowed prefix slice),
/// - The collected owned string is found as a substring of `original` (returns borrowed slice).
///
/// Otherwise, collect into an owned `String`.
pub fn cow(converted: impl IntoIterator<Item = char>, original: &str) -> Cow<'_, str> {
    let mut converted = converted.into_iter();
    let mut orig_chars = original.chars();
    let mut byte_offset = 0;

    loop {
        match (converted.next(), orig_chars.next()) {
            (None, None) => return Cow::Borrowed(original),
            (None, Some(_)) => return Cow::Borrowed(&original[..byte_offset]),
            (Some(conv), Some(orig)) if conv == orig => {
                byte_offset += orig.len_utf8();
            }
            (Some(conv), orig_opt) => {
                // Mismatch or original exhausted: collect the rest into an owned String.
                let prefix = &original[..byte_offset];
                let mut buf = String::with_capacity(original.len());
                buf.push_str(prefix);
                if let Some(orig) = orig_opt {
                    // We had a mismatch, not exhaustion: push the differing conv char.
                    buf.push(conv);
                    // Now skip the orig char's bytes (we don't push it).
                    let _ = orig;
                } else {
                    // Original was exhausted; push the conv char that went past.
                    buf.push(conv);
                }
                buf.extend(converted);
                return if let Some(pos) = original.find(buf.as_str()) {
                    Cow::Borrowed(&original[pos..pos + buf.len()])
                } else {
                    Cow::Owned(buf)
                };
            }
        }
    }
}

pub enum SubstringOrOwned<T: ?Sized + Segment> {
    Substring(usize, usize),
    Owned(<T as ToOwned>::Owned),
}

impl<T: ?Sized + Segment> Clone for SubstringOrOwned<T>
where
    <T as ToOwned>::Owned: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Substring(ofs, len) => Self::Substring(*ofs, *len),
            Self::Owned(s) => Self::Owned(s.clone()),
        }
    }
}

impl<T: ?Sized + Segment> core::fmt::Debug for SubstringOrOwned<T>
where
    <T as ToOwned>::Owned: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Substring(ofs, len) => f.debug_tuple("Substring").field(ofs).field(len).finish(),
            Self::Owned(s) => f.debug_tuple("Owned").field(s).finish(),
        }
    }
}

impl<T: ?Sized + Segment> SubstringOrOwned<T>
where
    <T as ToOwned>::Owned: Borrow<T>,
{
    /// If `value` is a substring of `original`, return `Substring`; otherwise `Owned`.
    pub fn new(value: &T, original: &T) -> Self {
        let value_bytes = value.as_byte_slice();
        let original_bytes = original.as_byte_slice();

        // Fast path: pointer overlap check.
        let original_start = original_bytes.as_ptr() as usize;
        let value_start = value_bytes.as_ptr() as usize;
        if value_start >= original_start
            && value_start + value_bytes.len() <= original_start + original_bytes.len()
        {
            return Self::Substring(value_start - original_start, value_bytes.len());
        }
        // Slow path: search for value content within original.
        if let Some(offset) = original.find_subslice(value) {
            Self::Substring(offset, value_bytes.len())
        } else {
            Self::Owned(value.to_owned())
        }
    }

    /// Returns `true` if this is a `Substring` spanning the entire original.
    pub fn is_identity(&self, original: &T) -> bool {
        matches!(self, Self::Substring(0, len) if *len == original.as_byte_slice().len())
    }

    pub fn as_ref<'a>(&'a self, original: &'a T) -> &'a T {
        match self {
            Self::Substring(ofs, len) => &original[*ofs..*ofs + *len],
            Self::Owned(s) => s.borrow(),
        }
    }

    pub fn into_cow(self, original: Cow<'_, T>) -> Cow<'_, T> {
        match self {
            Self::Owned(s) => Cow::Owned(s),
            Self::Substring(ofs, len) => {
                if ofs == 0 && len == original.as_byte_slice().len() {
                    original
                } else if let Cow::Borrowed(s) = original {
                    Cow::Borrowed(&s[ofs..ofs + len])
                } else {
                    Cow::Owned(original[ofs..ofs + len].to_owned())
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use alloc::string::ToString;
    use alloc::vec;

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{
        SubstringOrOwned, cow, decode_utf8_lossy, str_cow_to_bytes, to_java_modified_utf8,
    };

    // --- str_cow_to_bytes ---

    #[test]
    fn str_cow_to_bytes_borrowed() {
        let input = Cow::Borrowed("hello");
        let result = str_cow_to_bytes(input);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), b"hello");
    }

    #[test]
    fn str_cow_to_bytes_owned() {
        let input = Cow::Owned("hello".to_string());
        let result = str_cow_to_bytes(input);
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result.as_ref(), b"hello");
    }

    #[test]
    fn str_cow_to_bytes_empty() {
        let result = str_cow_to_bytes(Cow::Borrowed(""));
        assert_eq!(result.as_ref(), b"");
    }

    #[test]
    fn str_cow_to_bytes_multibyte() {
        let result = str_cow_to_bytes(Cow::Borrowed("é"));
        assert_eq!(result.as_ref(), "é".as_bytes());
    }

    // --- cow ---

    #[test]
    fn cow_identical_borrows() {
        let s = "hello";
        let result = cow(s.chars(), s);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn cow_empty_borrows() {
        let s = "";
        let result = cow(s.chars(), s);
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn cow_first_char_differs() {
        let result = cow("Hello".chars(), "hello");
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "Hello");
    }

    #[test]
    fn cow_last_char_differs() {
        let result = cow("hello".chars(), "hellO");
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn cow_middle_char_differs() {
        let result = cow("hello".chars(), "heLlo");
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn cow_converted_longer() {
        let result = cow("abcde".chars(), "abc");
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "abcde");
    }

    #[test]
    fn cow_converted_shorter_prefix_borrows() {
        let s = "abcde";
        let result = cow("abc".chars(), s);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "abc");
        assert!(core::ptr::eq(result.as_ptr(), s.as_ptr()));
    }

    #[test]
    fn cow_converted_shorter_mismatch_owns() {
        let result = cow("aXc".chars(), "abcde");
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "aXc");
    }

    #[test]
    fn cow_unicode_identical_borrows() {
        let s = "日本語";
        let result = cow(s.chars(), s);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "日本語");
    }

    #[test]
    fn cow_unicode_differs() {
        let result = cow("日本人".chars(), "日本語");
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "日本人");
    }

    #[test]
    fn cow_single_char_identical() {
        let s = "x";
        let result = cow(s.chars(), s);
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn cow_single_char_differs() {
        let result = cow("y".chars(), "x");
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "y");
    }

    #[test]
    fn cow_original_empty_converted_nonempty() {
        let result = cow("abc".chars(), "");
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "abc");
    }

    #[test]
    fn cow_original_nonempty_converted_empty() {
        let s = "abc";
        let result = cow("".chars(), s);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "");
    }

    #[test]
    fn cow_multibyte_expansion() {
        // Control char (1 byte) mapped to control picture (3 bytes)
        let result = cow("\u{2401}".chars(), "\x01");
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "\u{2401}");
    }

    #[test]
    fn cow_suffix_substring_borrows() {
        // Converted drops leading char, result is a suffix of original.
        let s = "abc";
        let result = cow("bc".chars(), s);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "bc");
        assert!(core::ptr::eq(result.as_ptr(), s[1..].as_ptr()));
    }

    #[test]
    fn cow_middle_substring_borrows() {
        // Converted is a middle substring of original.
        let s = "abcde";
        let result = cow("bcd".chars(), s);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "bcd");
        assert!(core::ptr::eq(result.as_ptr(), s[1..].as_ptr()));
    }

    #[test]
    fn cow_not_a_substring_owns() {
        let result = cow("xyz".chars(), "abc");
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "xyz");
    }

    #[test]
    fn cow_empty_converted_empty_original_borrows() {
        let s = "";
        let result = cow("".chars(), s);
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    // --- SubstringOrOwned<str> ---

    #[test]
    fn soo_str_new_substring() {
        let original = "hello world";
        let soo = SubstringOrOwned::<str>::new(&original[6..], original);
        assert!(matches!(soo, SubstringOrOwned::Substring(6, 5)));
        assert_eq!(soo.as_ref(original), "world");
    }

    #[test]
    fn soo_str_new_full() {
        let original = "hello";
        let soo = SubstringOrOwned::<str>::new(original, original);
        assert!(matches!(soo, SubstringOrOwned::Substring(0, 5)));
        assert_eq!(soo.as_ref(original), "hello");
    }

    #[test]
    fn soo_str_new_not_in_parent() {
        let original = "hello";
        let soo = SubstringOrOwned::<str>::new("xyz", original);
        assert!(matches!(soo, SubstringOrOwned::Owned(_)));
        assert_eq!(soo.as_ref(original), "xyz");
    }

    #[test]
    fn soo_str_new_different_allocation_content_matches() {
        let original = "hello";
        let other = "hello".to_string();
        let soo = SubstringOrOwned::<str>::new(&other, original);
        assert!(matches!(soo, SubstringOrOwned::Substring(0, 5)));
        assert_eq!(soo.as_ref(original), "hello");
    }

    #[test]
    fn soo_str_new_content_is_substring_of_parent() {
        let original = "hello world";
        let other = "world".to_string();
        let soo = SubstringOrOwned::<str>::new(&other, original);
        assert!(matches!(soo, SubstringOrOwned::Substring(6, 5)));
        assert_eq!(soo.as_ref(original), "world");
    }

    #[test]
    fn soo_str_into_cow_owned() {
        let soo = SubstringOrOwned::<str>::Owned("world".to_string());
        let result = soo.into_cow(Cow::Borrowed("hello"));
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "world");
    }

    #[test]
    fn soo_str_into_cow_substring_full_borrowed() {
        let original = "hello";
        let soo = SubstringOrOwned::<str>::Substring(0, 5);
        let result = soo.into_cow(Cow::Borrowed(original));
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn soo_str_into_cow_substring_partial_borrowed() {
        let original = "hello world";
        let soo = SubstringOrOwned::<str>::Substring(6, 5);
        let result = soo.into_cow(Cow::Borrowed(original));
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "world");
    }

    #[test]
    fn soo_str_into_cow_substring_from_owned_parent() {
        let soo = SubstringOrOwned::<str>::Substring(6, 5);
        let result = soo.into_cow(Cow::Owned("hello world".to_string()));
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result, "world");
    }

    // --- SubstringOrOwned<[u8]> ---

    #[test]
    fn soo_bytes_new_substring() {
        let original: &[u8] = b"hello world";
        let soo = SubstringOrOwned::<[u8]>::new(&original[6..], original);
        assert!(matches!(soo, SubstringOrOwned::Substring(6, 5)));
        assert_eq!(soo.as_ref(original), b"world");
    }

    #[test]
    fn soo_bytes_new_full() {
        let original: &[u8] = b"hello";
        let soo = SubstringOrOwned::<[u8]>::new(original, original);
        assert!(matches!(soo, SubstringOrOwned::Substring(0, 5)));
        assert_eq!(soo.as_ref(original), b"hello");
    }

    #[test]
    fn soo_bytes_new_not_in_parent() {
        let original: &[u8] = b"hello";
        let soo = SubstringOrOwned::<[u8]>::new(b"xyz", original);
        assert!(matches!(soo, SubstringOrOwned::Owned(_)));
        assert_eq!(soo.as_ref(original), b"xyz");
    }

    #[test]
    fn soo_bytes_new_content_matches() {
        let original: &[u8] = b"hello world";
        let other = b"world".to_vec();
        let soo = SubstringOrOwned::<[u8]>::new(&other, original);
        assert!(matches!(soo, SubstringOrOwned::Substring(6, 5)));
        assert_eq!(soo.as_ref(original), b"world");
    }

    #[test]
    fn soo_bytes_into_cow_owned() {
        let soo = SubstringOrOwned::<[u8]>::Owned(vec![1, 2, 3]);
        let result = soo.into_cow(Cow::Borrowed(b"hello" as &[u8]));
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result.as_ref(), &[1, 2, 3]);
    }

    #[test]
    fn soo_bytes_into_cow_substring_full_borrowed() {
        let original: &[u8] = b"hello";
        let soo = SubstringOrOwned::<[u8]>::Substring(0, 5);
        let result = soo.into_cow(Cow::Borrowed(original));
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), b"hello");
    }

    #[test]
    fn soo_bytes_into_cow_substring_partial_borrowed() {
        let original: &[u8] = b"hello world";
        let soo = SubstringOrOwned::<[u8]>::Substring(6, 5);
        let result = soo.into_cow(Cow::Borrowed(original));
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), b"world");
    }

    #[test]
    fn soo_bytes_into_cow_substring_from_owned_parent() {
        let soo = SubstringOrOwned::<[u8]>::Substring(6, 5);
        let result = soo.into_cow(Cow::Owned(b"hello world".to_vec()));
        assert!(matches!(result, Cow::Owned(_)));
        assert_eq!(result.as_ref(), b"world");
    }

    #[test]
    fn soo_bytes_is_identity() {
        let original: &[u8] = b"hello";
        let soo = SubstringOrOwned::<[u8]>::Substring(0, 5);
        assert!(soo.is_identity(original));
        let soo2 = SubstringOrOwned::<[u8]>::Substring(1, 4);
        assert!(!soo2.is_identity(original));
    }

    #[test]
    fn soo_str_is_identity() {
        let original = "hello";
        let soo = SubstringOrOwned::<str>::Substring(0, 5);
        assert!(soo.is_identity(original));
        let soo2 = SubstringOrOwned::<str>::Substring(1, 4);
        assert!(!soo2.is_identity(original));
    }

    // --- decode_lossy ---

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
        // U+1F600 (😀) is a valid 4-byte UTF-8 sequence
        let result = decode_utf8_lossy("😀".as_bytes());
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "😀");
    }

    #[test]
    fn decode_lossy_overlong_null() {
        // 0xC0 0x80 is an overlong encoding, replaced with U+FFFD per byte
        let result = decode_utf8_lossy(&[0xC0, 0x80]);
        assert_eq!(result, "\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn decode_lossy_overlong_null_in_context() {
        // "a" + overlong null + "b" — overlong replaced per byte
        let result = decode_utf8_lossy(&[0x61, 0xC0, 0x80, 0x62]);
        assert_eq!(result, "a\u{FFFD}\u{FFFD}b");
    }

    #[test]
    fn decode_lossy_cesu8_surrogate_pair() {
        // U+10000 (𐀀) encoded as CESU-8: ED A0 80 ED B0 80
        // High surrogate U+D800: ED A0 80
        // Low surrogate U+DC00: ED B0 80
        let result = decode_utf8_lossy(&[0xED, 0xA0, 0x80, 0xED, 0xB0, 0x80]);
        assert_eq!(result, "\u{10000}");
    }

    #[test]
    fn decode_lossy_cesu8_emoji() {
        // U+1F600 (😀) as CESU-8 surrogate pair:
        // U+D83D → ED A0 BD, U+DE00 → ED B8 80
        let result = decode_utf8_lossy(&[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
        assert_eq!(result, "😀");
    }

    #[test]
    fn decode_lossy_cesu8_in_context() {
        // "hi" + U+1F600 as CESU-8 + "!"
        let result = decode_utf8_lossy(&[0x68, 0x69, 0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80, 0x21]);
        assert_eq!(result, "hi😀!");
    }

    #[test]
    fn decode_lossy_lone_high_surrogate() {
        // High surrogate U+D800 without low surrogate — each byte replaced
        let result = decode_utf8_lossy(&[0xED, 0xA0, 0x80, 0x61]);
        assert_eq!(result, "\u{FFFD}\u{FFFD}\u{FFFD}a");
    }

    #[test]
    fn decode_lossy_lone_low_surrogate() {
        // Low surrogate U+DC00 without high surrogate — each byte replaced
        let result = decode_utf8_lossy(&[0xED, 0xB0, 0x80]);
        assert_eq!(result, "\u{FFFD}\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn decode_lossy_high_surrogate_followed_by_non_surrogate() {
        // High surrogate followed by regular 3-byte char, not low surrogate
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
        // 0xC3 without valid continuation — leader replaced, null passes through
        let result = decode_utf8_lossy(&[0xC3, 0x00]);
        assert_eq!(result, "\u{FFFD}\0");
    }

    #[test]
    fn decode_lossy_overlong_2byte_rejected() {
        // 0xC0 0xBF is overlong for U+003F ('?') — only 0xC0 0x80 is special
        let result = decode_utf8_lossy(&[0xC0, 0xBF]);
        assert_eq!(result, "\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn decode_lossy_overlong_3byte_rejected() {
        // Overlong 3-byte encoding of U+007F
        let result = decode_utf8_lossy(&[0xE0, 0x81, 0xBF]);
        assert_eq!(result, "\u{FFFD}\u{FFFD}\u{FFFD}");
    }

    #[test]
    fn decode_lossy_truncated_2byte() {
        // 0xC3 at end of input
        let result = decode_utf8_lossy(&[0x61, 0xC3]);
        assert_eq!(result, "a\u{FFFD}");
    }

    #[test]
    fn decode_lossy_truncated_3byte() {
        // 0xE3 0x81 at end of input — single replacement per from_utf8_lossy
        let result = decode_utf8_lossy(&[0x61, 0xE3, 0x81]);
        assert_eq!(result, "a\u{FFFD}");
    }

    #[test]
    fn decode_lossy_truncated_4byte() {
        // 0xF0 0x9F 0x98 at end of input — single replacement per from_utf8_lossy
        let result = decode_utf8_lossy(&[0x61, 0xF0, 0x9F, 0x98]);
        assert_eq!(result, "a\u{FFFD}");
    }

    #[test]
    fn decode_lossy_mixed_invalid_with_cesu8() {
        // CESU-8 pair + invalid byte: simd_cesu8::decode fails (invalid byte),
        // falls back to from_utf8_lossy which replaces the surrogate pair bytes too.
        let mut input = vec![0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]; // 😀 as CESU-8
        input.push(0xFF); // invalid
        let result = decode_utf8_lossy(&input);
        // from_utf8_lossy doesn't understand CESU-8 — all bytes invalid
        assert_eq!(
            result,
            "\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}"
        );
    }

    #[test]
    fn decode_lossy_pure_cesu8() {
        // Pure CESU-8 without invalid bytes — simd_cesu8::decode succeeds
        let mut input = vec![0x61]; // 'a'
        input.extend_from_slice(&[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]); // 😀 as CESU-8
        input.push(0x62); // 'b'
        let result = decode_utf8_lossy(&input);
        assert_eq!(result, "a😀b");
    }

    #[test]
    fn decode_lossy_utf8_emoji_plus_invalid() {
        // Valid UTF-8 emoji (4 bytes) + invalid byte: simd_cesu8::decode fails,
        // falls back to from_utf8_lossy which preserves the emoji.
        let mut input = "😀".as_bytes().to_vec();
        input.push(0xFF);
        let result = decode_utf8_lossy(&input);
        assert_eq!(result, "😀\u{FFFD}");
    }

    // --- to_java_modified_utf8 ---

    #[test]
    fn cesu8_ascii_borrows() {
        let result = to_java_modified_utf8("hello");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), b"hello");
    }

    #[test]
    fn cesu8_empty_borrows() {
        let result = to_java_modified_utf8("");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn cesu8_bmp_borrows() {
        let result = to_java_modified_utf8("éàü");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), "éàü".as_bytes());
    }

    #[test]
    fn cesu8_null_passthrough() {
        let result = to_java_modified_utf8("\0");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), &[0x00]);
    }

    #[test]
    fn cesu8_null_in_context() {
        let result = to_java_modified_utf8("a\0b");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), &[0x61, 0x00, 0x62]);
    }

    #[test]
    fn cesu8_supplementary_as_surrogate_pair() {
        // U+1F600 (😀) → surrogate pair U+D83D U+DE00
        let result = to_java_modified_utf8("😀");
        assert_eq!(result.as_ref(), &[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]);
    }

    #[test]
    fn cesu8_supplementary_in_context() {
        let result = to_java_modified_utf8("hi😀!");
        let mut expected = vec![0x68, 0x69]; // "hi"
        expected.extend_from_slice(&[0xED, 0xA0, 0xBD, 0xED, 0xB8, 0x80]); // 😀
        expected.push(0x21); // "!"
        assert_eq!(result.as_ref(), &expected);
    }

    #[test]
    fn cesu8_u10000() {
        // U+10000 → surrogates U+D800 U+DC00
        let result = to_java_modified_utf8("\u{10000}");
        assert_eq!(result.as_ref(), &[0xED, 0xA0, 0x80, 0xED, 0xB0, 0x80]);
    }

    #[test]
    fn cesu8_roundtrip_with_decode() {
        for s in &["hello", "a\0b", "😀", "\u{10000}", "hi😀\0world"] {
            let encoded = to_java_modified_utf8(s);
            let decoded = decode_utf8_lossy(&encoded);
            assert_eq!(&*decoded, *s, "roundtrip failed for {s:?}");
        }
    }
}
