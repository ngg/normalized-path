use alloc::borrow::Cow;
use alloc::borrow::ToOwned;
use alloc::string::String;
use core::borrow::Borrow;
use core::ops::{Index, Range};

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

    use super::{SubstringOrOwned, cow};

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
}
