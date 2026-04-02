use alloc::borrow::Cow;

use crate::ErrorKind;
use crate::error::ResultKind;
use crate::unicode::{case_fold, is_starter, is_whitespace, nfc, nfd};
use crate::utils::cow;

/// `White_Space` property check extended with Control Pictures (U+2409–U+240D) that
/// correspond to whitespace control characters (HT, LF, VT, FF, CR), and the BOM (U+FEFF).
#[must_use]
pub fn is_whitespace_like(c: char) -> bool {
    is_whitespace(c) || ('\u{2409}'..='\u{240D}').contains(&c) || c == '\u{FEFF}'
}

/// Map Fullwidth characters (U+FF01..U+FF5E) to their ASCII equivalents.
#[must_use]
pub fn map_fullwidth(s: &str) -> Cow<'_, str> {
    cow(
        s.chars().map(|c| match c {
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            _ => c,
        }),
        s,
    )
}

/// Trim leading and trailing `White_Space` characters, control pictures, and BOMs.
pub fn trim_whitespace_like(s: &str) -> &str {
    s.trim_matches(is_whitespace_like)
}

/// Map Turkish İ (U+0130) and ı (U+0131) to their ASCII
/// equivalents I and i. Only applied in case-insensitive mode, after case folding.
///
/// These characters are problematic for case-insensitive matching because Unicode
/// `toCasefold()` treats ı as distinct from i (ı → ı, not ı → i), yet locale-independent
/// `toUppercase` maps ı → I which case-folds to i. Mapping both to ASCII after case
/// folding ensures consistent behavior regardless of casing operations.
///
/// Additionally, U+0307 COMBINING DOT ABOVE is stripped after I/i (case-insensitive)
/// to handle NFD decomposition of İ (I + U+0307), with intervening combiners allowed.
#[must_use]
pub fn map_turkish_i(s: &str) -> Cow<'_, str> {
    cow(
        s.chars()
            .scan(false, |strip_dot, c| {
                match c {
                    '\u{0130}' | 'I' => {
                        // İ → I, strip following dot above
                        *strip_dot = true;
                        Some(Some('I'))
                    }
                    '\u{0131}' | 'i' => {
                        // ı → i, strip following dot above
                        *strip_dot = true;
                        Some(Some('i'))
                    }
                    '\u{0307}' if *strip_dot => {
                        // Strip combining dot above after I/i
                        Some(None)
                    }
                    _ => {
                        if is_starter(c) {
                            *strip_dot = false;
                        }
                        Some(Some(c))
                    }
                }
            })
            .flatten(),
        s,
    )
}

/// Map control characters to Unicode Control Pictures.
/// 0x01-0x1F → U+2401-U+241F, 0x7F → U+2421.
/// Null bytes (0x00) are excluded — they are rejected by validation instead.
#[must_use]
pub fn map_control_chars(s: &str) -> Cow<'_, str> {
    cow(
        s.chars().map(|c| match c {
            '\x01'..='\x1F' => char::from_u32(c as u32 + 0x2400).unwrap_or(c),
            '\x7F' => '\u{2421}',
            _ => c,
        }),
        s,
    )
}

/// Normalize a plaintext path element name case-sensitively.
///
/// Pipeline: NFD → whitespace trimming → fullwidth mapping →
/// control char mapping → validation → NFC.
///
/// # Errors
/// Returns an error if the name is invalid.
pub fn normalize_cs(name: &str) -> ResultKind<Cow<'_, str>> {
    let s = nfd(name);
    let s = trim_whitespace_like(&s);
    let s = map_fullwidth(s);
    let s = map_control_chars(&s);
    validate_path_element(&s)?;
    let s = nfc(&s);
    debug_assert!(validate_path_element(&s).is_ok());
    Ok(cow(s.chars(), name))
}

/// Derive the case-insensitive normalized form from an already case-sensitive normalized name.
///
/// Applies case folding, Turkish İ mapping, and NFC to a CS-normalized name.
/// Skips the steps already applied by CS normalization (trim, fullwidth, control chars).
#[must_use]
pub fn normalize_ci_from_normalized_cs(cs_normalized: &str) -> Cow<'_, str> {
    let s = nfd(cs_normalized);
    let s = case_fold(&s);
    let s = map_turkish_i(&s);
    let s = nfc(&s);
    debug_assert!(validate_path_element(&s).is_ok());
    cow(s.chars(), cs_normalized)
}

/// Validate a normalized path element name.
///
/// Rejects empty strings, `.`, `..`, names containing `/`, and names containing `\0`.
///
/// # Errors
/// Returns an error if the name is invalid.
pub fn validate_path_element(name: &str) -> ResultKind<()> {
    match name {
        "" => Err(ErrorKind::Empty),
        "." => Err(ErrorKind::CurrentDirectoryMarker),
        ".." => Err(ErrorKind::ParentDirectoryMarker),
        _ if name.contains('\0') => Err(ErrorKind::ContainsNullByte),
        _ if name.contains('/') => Err(ErrorKind::ContainsForwardSlash),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use alloc::string::String;

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{
        map_control_chars, map_fullwidth, map_turkish_i, normalize_ci_from_normalized_cs,
        normalize_cs, trim_whitespace_like, validate_path_element,
    };
    use crate::ErrorKind;
    // --- trim_whitespace_like ---

    #[test]
    fn trim_whitespace_like_removes_trailing() {
        assert_eq!(trim_whitespace_like("hello   "), "hello");
    }

    #[test]
    fn trim_whitespace_like_removes_leading() {
        assert_eq!(trim_whitespace_like("   hello"), "hello");
    }

    #[test]
    fn trim_whitespace_like_removes_both() {
        assert_eq!(trim_whitespace_like("  hello  "), "hello");
    }

    #[test]
    fn trim_whitespace_like_no_whitespace() {
        assert_eq!(trim_whitespace_like("hello"), "hello");
    }

    #[test]
    fn trim_whitespace_like_middle_preserved() {
        assert_eq!(trim_whitespace_like("he llo"), "he llo");
    }

    #[test]
    fn trim_whitespace_like_empty() {
        assert_eq!(trim_whitespace_like(""), "");
    }

    #[test]
    fn trim_whitespace_like_only_spaces() {
        assert_eq!(trim_whitespace_like("   "), "");
    }

    #[test]
    fn trim_whitespace_like_tabs() {
        assert_eq!(trim_whitespace_like("\thello\t"), "hello");
    }

    #[test]
    fn trim_whitespace_like_ideographic_space() {
        assert_eq!(trim_whitespace_like("\u{3000}hello\u{3000}"), "hello");
    }

    #[test]
    fn trim_whitespace_like_mixed() {
        assert_eq!(trim_whitespace_like("\t\u{3000} hello \t\u{3000}"), "hello");
    }

    #[test]
    fn trim_whitespace_like_control_picture_tab() {
        assert_eq!(trim_whitespace_like("\u{2409}hello\u{2409}"), "hello");
    }

    #[test]
    fn trim_whitespace_like_control_picture_lf() {
        assert_eq!(trim_whitespace_like("\u{240A}hello\u{240A}"), "hello");
    }

    #[test]
    fn trim_whitespace_like_control_picture_cr() {
        assert_eq!(trim_whitespace_like("\u{240D}hello\u{240D}"), "hello");
    }

    #[test]
    fn trim_whitespace_like_control_picture_middle_preserved() {
        assert_eq!(trim_whitespace_like("he\u{2409}llo"), "he\u{2409}llo");
    }

    #[test]
    fn trim_whitespace_like_bom_leading() {
        assert_eq!(trim_whitespace_like("\u{FEFF}hello"), "hello");
    }

    #[test]
    fn trim_whitespace_like_bom_trailing() {
        assert_eq!(trim_whitespace_like("hello\u{FEFF}"), "hello");
    }

    #[test]
    fn trim_whitespace_like_bom_both() {
        assert_eq!(trim_whitespace_like("\u{FEFF}hello\u{FEFF}"), "hello");
    }

    #[test]
    fn trim_whitespace_like_bom_middle_preserved() {
        assert_eq!(trim_whitespace_like("he\u{FEFF}llo"), "he\u{FEFF}llo");
    }

    #[test]
    fn trim_whitespace_like_only_bom() {
        assert_eq!(trim_whitespace_like("\u{FEFF}"), "");
    }

    #[test]
    fn trim_whitespace_like_multiple_leading_bom() {
        assert_eq!(
            trim_whitespace_like("\u{FEFF}\u{FEFF}\u{FEFF}hello"),
            "hello"
        );
    }

    // --- map_fullwidth ---

    #[test]
    fn map_fullwidth_letters() {
        assert_eq!(map_fullwidth("\u{FF21}\u{FF41}"), "Aa");
    }

    #[test]
    fn map_fullwidth_digits() {
        assert_eq!(map_fullwidth("\u{FF10}\u{FF19}"), "09");
    }

    #[test]
    fn map_fullwidth_symbols() {
        assert_eq!(map_fullwidth("\u{FF01}"), "!");
    }

    #[test]
    fn map_fullwidth_mixed() {
        assert_eq!(map_fullwidth("abc\u{FF21}def"), "abcAdef");
    }

    #[test]
    fn map_fullwidth_pure_ascii() {
        let result = map_fullwidth("hello");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn map_fullwidth_all_characters() {
        let fullwidth: String = ('\u{FF01}'..='\u{FF5E}').collect();
        let ascii: String = ('!'..='~').collect();
        assert_eq!(map_fullwidth(&fullwidth), ascii);
    }

    // --- map_control_chars ---

    #[test]
    fn map_control_del() {
        assert_eq!(map_control_chars("\x7F"), "\u{2421}");
    }

    #[test]
    fn map_control_normal_unchanged() {
        let result = map_control_chars("hello");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn map_control_mixed() {
        assert_eq!(map_control_chars("a\x01b\x7Fc"), "a\u{2401}b\u{2421}c");
    }

    #[test]
    fn map_control_null_byte_unchanged() {
        assert_eq!(map_control_chars("\x00"), "\x00");
    }

    #[test]
    fn map_control_all_c0_characters() {
        let controls: String = ('\x01'..='\x1F').collect();
        let pictures: String = ('\u{2401}'..='\u{241F}').collect();
        assert_eq!(map_control_chars(&controls), pictures);
    }

    // --- map_turkish_i ---

    #[test]
    fn map_turkish_i_dotted_capital() {
        assert_eq!(map_turkish_i("\u{0130}"), "I");
    }

    #[test]
    fn map_turkish_i_dotless_lowercase() {
        assert_eq!(map_turkish_i("\u{0131}"), "i");
    }

    #[test]
    fn map_turkish_i_dotless_lowercase_with_dot() {
        // ı followed by combining dot above: ı→i and strip the dot.
        // This handles Turkic fold output: fold_turkic("I\u{0307}") = "ı\u{0307}".
        assert_eq!(map_turkish_i("\u{0131}\u{0307}"), "i");
    }

    #[test]
    fn map_turkish_i_mixed() {
        assert_eq!(map_turkish_i("a\u{0130}b\u{0131}c"), "aIbic");
    }

    #[test]
    fn map_turkish_i_ascii_unchanged() {
        let result = map_turkish_i("Hello");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "Hello");
    }

    #[test]
    fn map_turkish_i_nfd_decomposed() {
        let result = map_turkish_i("I\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "I");
    }

    #[test]
    fn map_turkish_i_nfd_decomposed_lowercase() {
        let result = map_turkish_i("i\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "i");
    }

    #[test]
    fn map_turkish_i_intervening_combiner() {
        let result = map_turkish_i("I\u{0327}\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "I\u{0327}");
    }

    #[test]
    fn map_turkish_i_intervening_combiner_lowercase() {
        let result = map_turkish_i("i\u{0327}\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "i\u{0327}");
    }

    #[test]
    fn map_turkish_i_multiple_dots() {
        let result = map_turkish_i("I\u{0307}\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "I");
    }

    #[test]
    fn map_turkish_i_dot_on_other_base() {
        let result = map_turkish_i("e\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "e\u{0307}");
    }

    #[test]
    fn map_turkish_i_dot_after_starter_resets() {
        let result = map_turkish_i("Ia\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "Ia\u{0307}");
    }

    #[test]
    fn map_turkish_i_multiple_combiners_then_dot() {
        let result = map_turkish_i("i\u{0325}\u{0327}\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "i\u{0325}\u{0327}");
    }

    // --- normalize ---

    #[test]
    fn normalize_trims_leading_bom() {
        let input = "\u{FEFF}hello.txt";
        let with_bom = normalize_cs(input).unwrap();
        let without_bom = normalize_cs("hello.txt").unwrap();
        assert_eq!(with_bom, without_bom);
        assert!(matches!(with_bom, Cow::Borrowed(_)));
        assert!(matches!(without_bom, Cow::Borrowed(_)));
        assert!(core::ptr::eq(
            with_bom.as_ptr(),
            input["\u{FEFF}".len()..].as_ptr()
        ));
    }

    #[test]
    fn normalize_preserves_interior_bom() {
        let result = normalize_cs("he\u{FEFF}llo").unwrap();
        assert!(result.contains('\u{FEFF}'));
    }

    #[test]
    fn normalize_maps_fullwidth() {
        let fullwidth = normalize_cs("\u{FF21}bc.txt").unwrap();
        let ascii = normalize_cs("Abc.txt").unwrap();
        assert_eq!(fullwidth, ascii);
    }

    #[test]
    fn normalize_strips_whitespace() {
        let with_whitespace = normalize_cs("\t\u{3000} hello \t\u{3000}").unwrap();
        let without_whitespace = normalize_cs("hello").unwrap();
        assert_eq!(with_whitespace, without_whitespace);
        assert!(matches!(with_whitespace, Cow::Borrowed(_)));
        assert!(matches!(without_whitespace, Cow::Borrowed(_)));
    }

    #[test]
    fn normalize_trailing_whitespace_borrows_prefix() {
        let input = "hello   ";
        let result = normalize_cs(input).unwrap();
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
        assert!(core::ptr::eq(result.as_ptr(), input.as_ptr()));
    }

    #[test]
    fn normalize_leading_whitespace_borrows_suffix() {
        let input = "   hello";
        let result = normalize_cs(input).unwrap();
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
        assert!(core::ptr::eq(result.as_ptr(), input[3..].as_ptr()));
    }

    #[test]
    fn normalize_normalizes_unicode() {
        let nfd_input = normalize_cs("e\u{0301}.txt").unwrap();
        let composed = normalize_cs("\u{00E9}.txt").unwrap();
        assert_eq!(nfd_input, composed);
    }

    #[test]
    fn normalize_maps_control_chars() {
        let with_control = normalize_cs("a\x01b").unwrap();
        let with_picture = normalize_cs("a\u{2401}b").unwrap();
        assert_eq!(with_control, with_picture);
    }

    #[test]
    fn normalize_strips_whitespace_control_pictures() {
        let with_tab = normalize_cs("\thello").unwrap();
        let with_picture = normalize_cs("\u{2409}hello").unwrap();
        let plain = normalize_cs("hello").unwrap();
        assert_eq!(with_tab, plain);
        assert_eq!(with_picture, plain);
    }

    #[test]
    fn normalize_turkish_i_sensitive() {
        let ascii_upper = normalize_cs("I").unwrap();
        let ascii_lower = normalize_cs("i").unwrap();
        let dotted = normalize_cs("\u{0130}").unwrap();
        let dotless = normalize_cs("\u{0131}").unwrap();
        assert_eq!(ascii_upper, "I");
        assert_eq!(ascii_lower, "i");
        assert_eq!(dotted, "\u{0130}");
        assert_eq!(dotless, "\u{0131}");
        assert_ne!(ascii_upper, ascii_lower);
        assert_ne!(ascii_upper, dotted);
        assert_ne!(ascii_upper, dotless);
        assert_ne!(ascii_lower, dotted);
        assert_ne!(ascii_lower, dotless);
        assert_ne!(dotted, dotless);
    }

    // --- D145: U+0345 COMBINING GREEK YPOGEGRAMMENI ---

    #[test]
    fn normalize_ypogegrammeni_sensitive_preserved() {
        let cs = normalize_cs("\u{0345}").unwrap();
        assert_eq!(cs, "\u{0345}");
    }

    #[test]
    fn normalize_ypogegrammeni_with_overline_sensitive() {
        let a = normalize_cs("\u{0345}\u{0305}").unwrap();
        let b = normalize_cs("\u{0305}\u{0345}").unwrap();
        assert_eq!(a, b);
        assert_eq!(a, "\u{0305}\u{0345}");
    }

    // --- U+FB04 LATIN SMALL LIGATURE FFL ---

    #[test]
    fn normalize_ligature_ffl_sensitive() {
        let cs = normalize_cs("\u{FB04}").unwrap();
        assert_eq!(cs, "\u{FB04}");
    }

    // --- Supplementary plane case folding (Deseret) ---

    #[test]
    fn normalize_deseret_sensitive() {
        let upper = normalize_cs("\u{10400}").unwrap();
        let lower = normalize_cs("\u{10428}").unwrap();
        assert_ne!(upper, lower);
    }

    // --- Greek sigma ---

    #[test]
    fn normalize_greek_sigma_sensitive() {
        let small = normalize_cs("\u{03C3}").unwrap();
        let final_sigma = normalize_cs("\u{03C2}").unwrap();
        assert_ne!(small, final_sigma);
    }

    // --- Canonical equivalence ---

    #[test]
    fn normalize_ohm_sign_equals_omega() {
        let ohm = normalize_cs("\u{2126}").unwrap();
        let omega = normalize_cs("\u{03A9}").unwrap();
        assert_eq!(ohm, omega);
    }

    #[test]
    fn normalize_angstrom_equals_a_ring() {
        let angstrom = normalize_cs("\u{212B}").unwrap();
        let a_ring = normalize_cs("\u{00C5}").unwrap();
        assert_eq!(angstrom, a_ring);
    }

    // --- DZ digraph ---

    #[test]
    fn normalize_dz_digraph_sensitive() {
        let upper = normalize_cs("\u{01F1}").unwrap();
        let title = normalize_cs("\u{01F2}").unwrap();
        let lower = normalize_cs("\u{01F3}").unwrap();
        assert_ne!(upper, title);
        assert_ne!(upper, lower);
        assert_ne!(title, lower);
    }

    #[test]
    fn normalize_ascii_i_sensitive() {
        let upper = normalize_cs("I").unwrap();
        let lower = normalize_cs("i").unwrap();
        assert_ne!(upper, lower);
        assert_eq!(upper, "I");
        assert_eq!(lower, "i");
    }

    #[test]
    fn normalize_empty_rejected() {
        assert!(normalize_cs("").is_err());
    }

    #[test]
    fn normalize_dot_rejected() {
        assert!(normalize_cs(".").is_err());
    }

    #[test]
    fn normalize_dotdot_rejected() {
        assert!(normalize_cs("..").is_err());
    }

    #[test]
    fn normalize_slash_rejected() {
        assert!(normalize_cs("a/b").is_err());
    }

    #[test]
    fn normalize_bom_only_rejected() {
        assert!(normalize_cs("\u{FEFF}").is_err());
    }

    #[test]
    fn normalize_bom_dot_rejected() {
        assert!(normalize_cs("\u{FEFF}.").is_err());
    }

    // --- validate_path_element ---

    #[test]
    fn validate_empty_rejected() {
        assert!(validate_path_element("").is_err());
    }

    #[test]
    fn validate_dot_rejected() {
        assert!(validate_path_element(".").is_err());
    }

    #[test]
    fn validate_dotdot_rejected() {
        assert!(validate_path_element("..").is_err());
    }

    #[test]
    fn validate_slash_rejected() {
        assert!(validate_path_element("a/b").is_err());
    }

    #[test]
    fn validate_valid_path_element() {
        assert!(validate_path_element("hello.txt").is_ok());
    }

    #[test]
    fn validate_dotfile() {
        assert!(validate_path_element(".gitignore").is_ok());
    }

    #[test]
    fn validate_triple_dot() {
        assert!(validate_path_element("...").is_ok());
    }

    #[test]
    fn validate_unicode() {
        assert!(validate_path_element("日本語.txt").is_ok());
    }

    #[test]
    fn validate_null_byte_rejected() {
        assert!(matches!(
            validate_path_element("\0"),
            Err(ErrorKind::ContainsNullByte)
        ));
        assert!(matches!(
            validate_path_element("a\0b"),
            Err(ErrorKind::ContainsNullByte)
        ));
    }

    // --- normalize_cs ---

    #[test]
    fn normalize_cs_null_byte_rejected() {
        assert!(matches!(
            normalize_cs("a\0b"),
            Err(ErrorKind::ContainsNullByte)
        ));
    }

    #[test]
    fn normalize_sensitive_preserves_case() {
        let upper = normalize_cs("Hello.txt").unwrap();
        let lower = normalize_cs("hello.txt").unwrap();
        assert_ne!(upper, lower);
        assert!(matches!(upper, Cow::Borrowed(_)));
        assert!(matches!(lower, Cow::Borrowed(_)));
    }

    // --- normalize_ci_from_normalized_cs ---

    #[test]
    fn ci_from_cs_turkish_i() {
        assert_eq!(normalize_ci_from_normalized_cs("I"), "i");
        assert_eq!(normalize_ci_from_normalized_cs("i"), "i");
        assert_eq!(
            normalize_ci_from_normalized_cs(&normalize_cs("\u{0130}").unwrap()),
            "i"
        );
        assert_eq!(
            normalize_ci_from_normalized_cs(&normalize_cs("\u{0131}").unwrap()),
            "i"
        );
    }

    #[test]
    fn ci_from_cs_i_combining_dot() {
        // "I\u{0307}" NFC-composes to İ (U+0130), CS-normalized is "\u{0130}".
        // CI must map to "i".
        let cs = normalize_cs("I\u{0307}").unwrap();
        assert_eq!(normalize_ci_from_normalized_cs(&cs), "i");

        // "ı\u{0307}" — dotless i + combining dot → maps to "i".
        let cs = normalize_cs("\u{0131}\u{0307}").unwrap();
        assert_eq!(normalize_ci_from_normalized_cs(&cs), "i");
    }

    #[test]
    fn ci_from_cs_ypogegrammeni() {
        assert_eq!(normalize_ci_from_normalized_cs("\u{0345}"), "\u{03B9}");

        // With overline: order shouldn't matter after CS normalization.
        let a = normalize_cs("\u{0345}\u{0305}").unwrap();
        let b = normalize_cs("\u{0305}\u{0345}").unwrap();
        assert_eq!(
            normalize_ci_from_normalized_cs(&a),
            normalize_ci_from_normalized_cs(&b)
        );
    }

    #[test]
    fn ci_from_cs_composed_ypogegrammeni() {
        let cs_a = normalize_cs("\u{1FC3}").unwrap();
        let cs_b = normalize_cs("\u{03B7}\u{0345}").unwrap();
        assert_eq!(
            normalize_ci_from_normalized_cs(&cs_a),
            normalize_ci_from_normalized_cs(&cs_b)
        );
    }

    #[test]
    fn ci_from_cs_ligature_ffl() {
        // Ligature is preserved by CS normalization, then case-folded to "ffl".
        assert_eq!(
            normalize_ci_from_normalized_cs(&normalize_cs("\u{FB04}").unwrap()),
            "ffl"
        );
        assert_eq!(normalize_ci_from_normalized_cs("ffl"), "ffl");
        assert_eq!(normalize_ci_from_normalized_cs("FFL"), "ffl");
        assert_eq!(normalize_ci_from_normalized_cs("Ffl"), "ffl");
    }

    #[test]
    fn ci_from_cs_deseret() {
        let upper = normalize_ci_from_normalized_cs("\u{10400}");
        let lower = normalize_ci_from_normalized_cs("\u{10428}");
        assert_eq!(upper, lower);
    }

    #[test]
    fn ci_from_cs_greek_sigma() {
        let capital = normalize_ci_from_normalized_cs("\u{03A3}");
        let small = normalize_ci_from_normalized_cs("\u{03C3}");
        let final_s = normalize_ci_from_normalized_cs("\u{03C2}");
        assert_eq!(capital, small);
        assert_eq!(capital, final_s);

        let upper_cs = normalize_cs("ΛΌΓΟΣ").unwrap();
        let lower_cs = normalize_cs("λόγος").unwrap();
        let upper = normalize_ci_from_normalized_cs(&upper_cs);
        let lower = normalize_ci_from_normalized_cs(&lower_cs);
        assert_eq!(upper, lower);
    }

    #[test]
    fn ci_from_cs_ohm_omega() {
        // Ohm sign and Omega are canonically equivalent after NFC (CS step),
        // so they share the same CS-normalized form.
        let ohm_cs = normalize_cs("\u{2126}").unwrap();
        let omega_cs = normalize_cs("\u{03A9}").unwrap();
        assert_eq!(ohm_cs, omega_cs);
        let small_cs = normalize_cs("\u{03C9}").unwrap();
        assert_eq!(
            normalize_ci_from_normalized_cs(&ohm_cs),
            normalize_ci_from_normalized_cs(&small_cs)
        );
    }

    #[test]
    fn ci_from_cs_angstrom() {
        let angstrom_cs = normalize_cs("\u{212B}").unwrap();
        let upper_cs = normalize_cs("\u{00C5}").unwrap();
        let lower_cs = normalize_cs("\u{00E5}").unwrap();
        assert_eq!(angstrom_cs, upper_cs); // canonically equivalent
        assert_eq!(
            normalize_ci_from_normalized_cs(&angstrom_cs),
            normalize_ci_from_normalized_cs(&lower_cs)
        );
    }

    #[test]
    fn ci_from_cs_micro_sign() {
        let micro = normalize_ci_from_normalized_cs("\u{00B5}");
        let mu_small = normalize_ci_from_normalized_cs("\u{03BC}");
        let mu_capital = normalize_ci_from_normalized_cs("\u{039C}");
        assert_eq!(micro, mu_small);
        assert_eq!(micro, mu_capital);
    }

    #[test]
    fn ci_from_cs_dz_digraph() {
        let upper_cs = normalize_cs("\u{01F1}").unwrap();
        let title_cs = normalize_cs("\u{01F2}").unwrap();
        let lower_cs = normalize_cs("\u{01F3}").unwrap();
        let upper = normalize_ci_from_normalized_cs(&upper_cs);
        let title = normalize_ci_from_normalized_cs(&title_cs);
        let lower = normalize_ci_from_normalized_cs(&lower_cs);
        assert_eq!(upper, title);
        assert_eq!(upper, lower);
    }

    #[test]
    fn ci_from_cs_german_eszett() {
        let lower_cs = normalize_cs("stra\u{00DF}e").unwrap();
        let upper_cs = normalize_cs("STRASSE").unwrap();
        let lower = normalize_ci_from_normalized_cs(&lower_cs);
        let upper = normalize_ci_from_normalized_cs(&upper_cs);
        assert_eq!(lower, upper);
    }

    #[test]
    fn ci_from_cs_hello() {
        assert_eq!(normalize_ci_from_normalized_cs("Hello.txt"), "hello.txt");
        assert_eq!(normalize_ci_from_normalized_cs("hello.txt"), "hello.txt");
        assert_eq!(normalize_ci_from_normalized_cs("HELLO.TXT"), "hello.txt");
    }

    #[test]
    fn ci_from_cs_nfc_nfd_equivalent() {
        // NFC and NFD inputs produce the same CS-normalized form (NFC),
        // so ci_from_cs must give the same result.
        let nfc_cs = normalize_cs("\u{00C9}.txt").unwrap();
        let decomposed_cs = normalize_cs("E\u{0301}.txt").unwrap();
        assert_eq!(nfc_cs, decomposed_cs);
        assert_eq!(
            normalize_ci_from_normalized_cs(&nfc_cs),
            normalize_ci_from_normalized_cs(&decomposed_cs)
        );
    }

    #[test]
    fn ci_from_cs_japanese_unchanged() {
        let result = normalize_ci_from_normalized_cs("日本語.txt");
        assert_eq!(result.as_ref(), "日本語.txt");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn ci_from_cs_idempotent() {
        let first = normalize_ci_from_normalized_cs("Hello.txt");
        let second = normalize_ci_from_normalized_cs(&first);
        assert_eq!(first, second);
        assert!(matches!(second, Cow::Borrowed(_)));
    }

    #[test]
    fn ci_from_cs_already_folded_borrows() {
        let result = normalize_ci_from_normalized_cs("hello.txt");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result.as_ref(), "hello.txt");
    }
}
