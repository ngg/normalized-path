use alloc::borrow::Cow;

use crate::ErrorKind;
use crate::error::ResultKind;
use crate::unicode::{
    case_fold, is_above, is_assigned, is_control, is_soft_dotted, is_starter, is_whitespace, nfc,
    nfd,
};
use crate::utils::cow;

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

/// Post-case-fold fixup for casing inconsistencies.
/// Applied after `toCasefold()` in case-insensitive mode.
///
/// - Maps dotless ı (U+0131) to ASCII i.
///   `toCasefold()` treats ı as distinct from i, yet `toUppercase(ı)` = I
///   even without locale tailoring, creating collisions that folding alone misses.
/// - Strips U+0307 COMBINING DOT ABOVE after any `Soft_Dotted` character
///   (e.g. i, j, Cyrillic і/ј), blocked by intervening starters or CCC=230
///   Above combiners (matching the Unicode `After_Soft_Dotted` condition).
///   This handles the `i\u{0307}` output from `toCasefold(İ)` and Lithuanian
///   casing rules: lowercase adds U+0307 after I/J/Į when more accents are above
///   (e.g. `lt_lowercase("J\u{0301}")` = `j\u{0307}\u{0301}`), and upper/titlecase
///   removes U+0307 after soft-dotted characters
///   (e.g. `lt_uppercase("j\u{0307}")` = `J`).
///
/// Note: this function relies on the invariant that `toCasefold()` preserves
/// the `Soft_Dotted` property — every `Soft_Dotted` character either folds to
/// itself or to another `Soft_Dotted` character. The reverse is not true: some
/// non-`Soft_Dotted` characters fold to `Soft_Dotted` ones (e.g. I → i), but
/// that is harmless since it only adds extra dot stripping, not skips it.
/// This invariant is verified by a test in `unicode.rs`.
///
/// See <https://www.unicode.org/Public/17.0.0/ucd/SpecialCasing.txt>.
#[must_use]
pub fn fixup_case_fold(s: &str) -> Cow<'_, str> {
    cow(
        s.chars()
            .scan(false, |strip_dot_above, c| {
                Some(match c {
                    '\u{0131}' => {
                        // ı → i (i is Soft_Dotted)
                        *strip_dot_above = true;
                        Some('i')
                    }
                    _ if is_soft_dotted(c) => {
                        *strip_dot_above = true;
                        Some(c)
                    }
                    '\u{0307}' if *strip_dot_above => {
                        // Strip combining dot above after Soft_Dotted character
                        None
                    }
                    _ => {
                        // Reset on starters (CCC=0) or CCC=230 (Above), matching
                        // the `After_Soft_Dotted`, `More_Above`, `Before_Dot`, and
                        // `After_I` conditions in SpecialCasing.txt.
                        if is_starter(c) || is_above(c) {
                            *strip_dot_above = false;
                        }
                        Some(c)
                    }
                })
            })
            .flatten(),
        s,
    )
}

/// Normalize a plaintext path element name case-sensitively.
///
/// Pipeline: NFD → whitespace trimming → fullwidth mapping →
/// validation → NFC.
///
/// # Errors
/// Returns an error if the name is invalid.
pub fn normalize_cs(name: &str) -> ResultKind<Cow<'_, str>> {
    let s = nfd(name);
    let s = s.trim_matches(|c| is_whitespace(c) && !is_control(c));
    let s = map_fullwidth(s);
    validate_path_element(&s)?;
    let s = nfc(&s);
    debug_assert!(validate_path_element(&s).is_ok());
    Ok(cow(s.chars(), name))
}

/// Derive the case-insensitive normalized form from an already case-sensitive normalized name.
///
/// Applies NFD, case folding, post-case-fold fixup ([`fixup_case_fold()`]), and NFC
/// to a CS-normalized name. Skips the steps already applied by CS normalization
/// (trim, fullwidth, validation).
#[must_use]
pub fn normalize_ci_from_normalized_cs(cs_normalized: &str) -> Cow<'_, str> {
    let s = nfd(cs_normalized);
    let s = case_fold(&s);
    let s = fixup_case_fold(&s);
    let s = nfc(&s);
    debug_assert!(validate_path_element(&s).is_ok());
    cow(s.chars(), cs_normalized)
}

/// Validate a normalized path element name.
///
/// Rejects empty strings, `.`, `..`, names containing `/`, `\0`, control
/// Unicode `Control` characters, BOM (U+FEFF), or unassigned Unicode characters.
///
/// # Errors
/// Returns an error if the name is invalid.
pub fn validate_path_element(name: &str) -> ResultKind<()> {
    match name {
        "" => return Err(ErrorKind::Empty),
        "." => return Err(ErrorKind::CurrentDirectoryMarker),
        ".." => return Err(ErrorKind::ParentDirectoryMarker),
        _ => {}
    }
    for c in name.chars() {
        match c {
            '\0' => return Err(ErrorKind::ContainsNullByte),
            _ if is_control(c) => return Err(ErrorKind::ContainsControlCharacter),
            '\u{FEFF}' => return Err(ErrorKind::ContainsBom),
            '/' => return Err(ErrorKind::ContainsForwardSlash),
            _ if !is_assigned(c) => return Err(ErrorKind::ContainsUnassignedChar),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use alloc::string::String;

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{
        fixup_case_fold, map_fullwidth, normalize_ci_from_normalized_cs, normalize_cs,
        validate_path_element,
    };
    use crate::ErrorKind;

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

    // --- fixup_case_fold ---

    #[test]
    fn fixup_case_fold_dotless_lowercase() {
        assert_eq!(fixup_case_fold("\u{0131}"), "i");
    }

    #[test]
    fn fixup_case_fold_dotless_lowercase_with_dot() {
        // ı followed by combining dot above: ı→i and strip the dot.
        // This handles Turkic fold output: fold_turkic("I\u{0307}") = "ı\u{0307}".
        assert_eq!(fixup_case_fold("\u{0131}\u{0307}"), "i");
    }

    #[test]
    fn fixup_case_fold_mixed() {
        assert_eq!(fixup_case_fold("a\u{0131}b\u{0131}c"), "aibic");
    }

    #[test]
    fn fixup_case_fold_ascii_unchanged() {
        let result = fixup_case_fold("Hello");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "Hello");
    }

    #[test]
    fn fixup_case_fold_nfd_decomposed() {
        // After case_fold, İ decomposes to i + U+0307. The dot is stripped.
        let result = fixup_case_fold("i\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "i");
    }

    #[test]
    fn fixup_case_fold_intervening_combiner() {
        // U+0327 COMBINING CEDILLA has CCC=202 (not Above), so dot stripping proceeds.
        let result = fixup_case_fold("i\u{0327}\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "i\u{0327}");
    }

    #[test]
    fn fixup_case_fold_multiple_dots() {
        let result = fixup_case_fold("i\u{0307}\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "i");
    }

    #[test]
    fn fixup_case_fold_dot_on_other_base() {
        let result = fixup_case_fold("e\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "e\u{0307}");
    }

    #[test]
    fn fixup_case_fold_dot_after_starter_resets() {
        let result = fixup_case_fold("ia\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "ia\u{0307}");
    }

    #[test]
    fn fixup_case_fold_multiple_combiners_then_dot() {
        let result = fixup_case_fold("i\u{0325}\u{0327}\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "i\u{0325}\u{0327}");
    }

    #[test]
    fn fixup_case_fold_above_combiner_blocks_strip() {
        // U+0301 COMBINING ACUTE ACCENT has CCC=230 (Above), which blocks dot stripping.
        let result = fixup_case_fold("i\u{0301}\u{0307}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "i\u{0301}\u{0307}");
    }

    // --- fixup_case_fold: Lithuanian J dot stripping ---

    #[test]
    fn fixup_case_fold_j_dot_above_stripped() {
        // Lithuanian lowercase adds U+0307 after j.
        assert_eq!(fixup_case_fold("j\u{0307}"), "j");
    }

    #[test]
    fn fixup_case_fold_j_dot_with_circumflex() {
        // Lithuanian lowercase of Ĵ + accent: j + dot + circumflex → dot stripped
        assert_eq!(fixup_case_fold("j\u{0307}\u{0302}"), "j\u{0302}");
    }

    #[test]
    fn fixup_case_fold_j_no_dot_unchanged() {
        let result = fixup_case_fold("j\u{0302}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "j\u{0302}");
    }

    // --- fixup_case_fold: Soft_Dotted characters ---

    #[test]
    fn fixup_case_fold_cyrillic_i_dot_stripped() {
        // Cyrillic і (U+0456) has the Soft_Dotted property.
        assert_eq!(fixup_case_fold("\u{0456}\u{0307}"), "\u{0456}");
    }

    #[test]
    fn fixup_case_fold_cyrillic_je_dot_stripped() {
        // Cyrillic ј (U+0458) has the Soft_Dotted property.
        assert_eq!(fixup_case_fold("\u{0458}\u{0307}"), "\u{0458}");
    }

    #[test]
    fn fixup_case_fold_i_ogonek_dot_stripped() {
        // Latin į (U+012F) has the Soft_Dotted property.
        assert_eq!(fixup_case_fold("\u{012F}\u{0307}"), "\u{012F}");
    }

    // --- normalize ---

    #[test]
    fn normalize_rejects_leading_bom() {
        assert_eq!(
            normalize_cs("\u{FEFF}hello.txt"),
            Err(ErrorKind::ContainsBom)
        );
    }

    #[test]
    fn normalize_rejects_interior_bom() {
        assert_eq!(normalize_cs("he\u{FEFF}llo"), Err(ErrorKind::ContainsBom));
    }

    #[test]
    fn normalize_maps_fullwidth() {
        let fullwidth = normalize_cs("\u{FF21}bc.txt").unwrap();
        let ascii = normalize_cs("Abc.txt").unwrap();
        assert_eq!(fullwidth, ascii);
    }

    #[test]
    fn normalize_strips_whitespace() {
        let with_whitespace = normalize_cs("\u{3000} hello \u{3000}").unwrap();
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
    fn normalize_rejects_control_chars() {
        use alloc::format;
        // All Cc characters except null (which gets ContainsNullByte)
        for cp in (0x01..=0x1Fu32).chain(0x7F..=0x9F) {
            let c = char::from_u32(cp).unwrap();
            let input = format!("a{c}b");
            assert_eq!(
                normalize_cs(&input),
                Err(ErrorKind::ContainsControlCharacter),
                "expected ContainsControlCharacter for U+{cp:04X}"
            );
        }
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
        assert_eq!(normalize_cs(""), Err(ErrorKind::Empty));
    }

    #[test]
    fn normalize_dot_rejected() {
        assert_eq!(normalize_cs("."), Err(ErrorKind::CurrentDirectoryMarker));
    }

    #[test]
    fn normalize_dotdot_rejected() {
        assert_eq!(normalize_cs(".."), Err(ErrorKind::ParentDirectoryMarker));
    }

    #[test]
    fn normalize_slash_rejected() {
        assert_eq!(normalize_cs("a/b"), Err(ErrorKind::ContainsForwardSlash));
    }

    #[test]
    fn normalize_bom_only_rejected() {
        assert_eq!(normalize_cs("\u{FEFF}"), Err(ErrorKind::ContainsBom));
    }

    #[test]
    fn normalize_bom_dot_rejected() {
        assert_eq!(normalize_cs("\u{FEFF}."), Err(ErrorKind::ContainsBom));
    }

    // --- validate_path_element ---

    #[test]
    fn validate_empty_rejected() {
        assert_eq!(validate_path_element(""), Err(ErrorKind::Empty));
    }

    #[test]
    fn validate_dot_rejected() {
        assert_eq!(
            validate_path_element("."),
            Err(ErrorKind::CurrentDirectoryMarker)
        );
    }

    #[test]
    fn validate_dotdot_rejected() {
        assert_eq!(
            validate_path_element(".."),
            Err(ErrorKind::ParentDirectoryMarker)
        );
    }

    #[test]
    fn validate_slash_rejected() {
        assert_eq!(
            validate_path_element("a/b"),
            Err(ErrorKind::ContainsForwardSlash)
        );
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
        assert_eq!(
            validate_path_element("\0"),
            Err(ErrorKind::ContainsNullByte)
        );
        assert_eq!(
            validate_path_element("a\0b"),
            Err(ErrorKind::ContainsNullByte)
        );
    }

    #[test]
    fn validate_control_character_rejected() {
        use alloc::format;
        // All Cc characters except null (which gets ContainsNullByte)
        let controls = (0x01..=0x1Fu32).chain(0x7F..=0x9Fu32);
        for cp in controls {
            let c = char::from_u32(cp).unwrap();
            let input = format!("a{c}b");
            assert_eq!(
                validate_path_element(&input),
                Err(ErrorKind::ContainsControlCharacter),
                "expected ContainsControlCharacter for U+{cp:04X}"
            );
        }
    }

    #[test]
    fn validate_bom_rejected() {
        assert_eq!(
            validate_path_element("\u{FEFF}hello"),
            Err(ErrorKind::ContainsBom)
        );
        assert_eq!(
            validate_path_element("he\u{FEFF}llo"),
            Err(ErrorKind::ContainsBom)
        );
    }

    #[test]
    fn validate_unassigned_rejected() {
        assert_eq!(
            validate_path_element("a\u{0378}b"),
            Err(ErrorKind::ContainsUnassignedChar)
        );
    }

    #[test]
    fn validate_assigned_accepted() {
        assert!(validate_path_element("hello.txt").is_ok());
        assert!(validate_path_element("\u{1FAEA}").is_ok()); // DISTORTED FACE
    }

    // --- normalize_cs ---

    #[test]
    fn normalize_cs_unassigned_rejected() {
        assert_eq!(
            normalize_cs("a\u{0378}b"),
            Err(ErrorKind::ContainsUnassignedChar)
        );
    }

    #[test]
    fn normalize_cs_null_byte_rejected() {
        assert_eq!(normalize_cs("a\0b"), Err(ErrorKind::ContainsNullByte));
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
        // İ (U+0130) is already NFC
        assert_eq!(normalize_ci_from_normalized_cs("\u{0130}"), "i");
        // ı (U+0131) is already NFC
        assert_eq!(normalize_ci_from_normalized_cs("\u{0131}"), "i");
    }

    #[test]
    fn ci_from_cs_i_combining_dot() {
        // "I\u{0307}" NFC-composes to İ (U+0130). CI must map to "i".
        assert_eq!(normalize_ci_from_normalized_cs("\u{0130}"), "i");

        // "ı\u{0307}" stays as-is in NFC. CI must map to "i".
        assert_eq!(normalize_ci_from_normalized_cs("\u{0131}\u{0307}"), "i");
    }

    #[test]
    fn ci_from_cs_soft_dotted_dot_stripped() {
        // Cyrillic і (U+0456) is Soft_Dotted: dot above is stripped.
        assert_eq!(
            normalize_ci_from_normalized_cs("\u{0456}\u{0307}"),
            "\u{0456}"
        );
        // Cyrillic ј (U+0458) is Soft_Dotted: dot above is stripped.
        assert_eq!(
            normalize_ci_from_normalized_cs("\u{0458}\u{0307}"),
            "\u{0458}"
        );
        // Latin į (U+012F) is Soft_Dotted: dot above is stripped.
        assert_eq!(
            normalize_ci_from_normalized_cs("\u{012F}\u{0307}"),
            "\u{012F}"
        );
        // Greek yot (U+03F3) is Soft_Dotted: dot above is stripped.
        assert_eq!(
            normalize_ci_from_normalized_cs("\u{03F3}\u{0307}"),
            "\u{03F3}"
        );
    }

    #[test]
    fn ci_from_cs_non_soft_dotted_dot_preserved() {
        // e is NOT Soft_Dotted: dot above is preserved (and NFC-composes to ė).
        assert_eq!(normalize_ci_from_normalized_cs("e\u{0307}"), "\u{0117}");
    }

    #[test]
    fn ci_from_cs_ypogegrammeni() {
        assert_eq!(normalize_ci_from_normalized_cs("\u{0345}"), "\u{03B9}");

        // Both orderings CS-normalize to "\u{0305}\u{0345}" (overline CCC=230 < ypogegrammeni CCC=240).
        // Ypogegrammeni case-folds to ι (U+03B9).
        assert_eq!(
            normalize_ci_from_normalized_cs("\u{0305}\u{0345}"),
            "\u{0305}\u{03B9}"
        );
    }

    #[test]
    fn ci_from_cs_composed_ypogegrammeni() {
        // U+1FC3 (ᾳ) = η + ypogegrammeni → η + ι after case fold.
        assert_eq!(
            normalize_ci_from_normalized_cs("\u{1FC3}"),
            "\u{03B7}\u{03B9}"
        );
    }

    #[test]
    fn ci_from_cs_ligature_ffl() {
        // Ligature U+FB04 is preserved by CS normalization, then case-folded to "ffl".
        assert_eq!(normalize_ci_from_normalized_cs("\u{FB04}"), "ffl");
        assert_eq!(normalize_ci_from_normalized_cs("ffl"), "ffl");
        assert_eq!(normalize_ci_from_normalized_cs("FFL"), "ffl");
        assert_eq!(normalize_ci_from_normalized_cs("Ffl"), "ffl");
    }

    #[test]
    fn ci_from_cs_deseret() {
        assert_eq!(normalize_ci_from_normalized_cs("\u{10400}"), "\u{10428}");
        assert_eq!(normalize_ci_from_normalized_cs("\u{10428}"), "\u{10428}");
    }

    #[test]
    fn ci_from_cs_ohm_omega() {
        // Ohm sign (U+2126) and Omega (U+03A9) both CS-normalize to Ω (U+03A9).
        assert_eq!(normalize_ci_from_normalized_cs("\u{03A9}"), "\u{03C9}");
        assert_eq!(normalize_ci_from_normalized_cs("\u{03C9}"), "\u{03C9}");
    }

    #[test]
    fn ci_from_cs_angstrom() {
        // Angstrom (U+212B) and Å (U+00C5) both CS-normalize to Å (U+00C5).
        assert_eq!(normalize_ci_from_normalized_cs("\u{00C5}"), "\u{00E5}");
        assert_eq!(normalize_ci_from_normalized_cs("\u{00E5}"), "\u{00E5}");
    }

    #[test]
    fn ci_from_cs_micro_sign() {
        assert_eq!(normalize_ci_from_normalized_cs("\u{00B5}"), "\u{03BC}");
        assert_eq!(normalize_ci_from_normalized_cs("\u{03BC}"), "\u{03BC}");
        assert_eq!(normalize_ci_from_normalized_cs("\u{039C}"), "\u{03BC}");
    }

    #[test]
    fn ci_from_cs_dz_digraph() {
        // U+01F1 DZ, U+01F2 Dz, U+01F3 dz (ligatures) all fold to U+01F3.
        // The ASCII pairs "DZ"/"dz" fold to "dz" instead — they are distinct.
        assert_eq!(normalize_ci_from_normalized_cs("\u{01F1}"), "\u{01F3}");
        assert_eq!(normalize_ci_from_normalized_cs("\u{01F2}"), "\u{01F3}");
        assert_eq!(normalize_ci_from_normalized_cs("\u{01F3}"), "\u{01F3}");
    }

    #[test]
    fn ci_from_cs_sharp_s_variants() {
        // All sharp s and "ss" variants normalize to "ss".
        assert_eq!(normalize_ci_from_normalized_cs("ss"), "ss");
        assert_eq!(normalize_ci_from_normalized_cs("SS"), "ss");
        assert_eq!(normalize_ci_from_normalized_cs("sS"), "ss");
        assert_eq!(normalize_ci_from_normalized_cs("Ss"), "ss");
        assert_eq!(normalize_ci_from_normalized_cs("\u{00DF}"), "ss"); // ß
        assert_eq!(normalize_ci_from_normalized_cs("\u{1E9E}"), "ss"); // ẞ
    }

    #[test]
    fn ci_from_cs_greek_sigma_variants() {
        // All sigma variants normalize to σ (U+03C3).
        assert_eq!(normalize_ci_from_normalized_cs("\u{03A3}"), "\u{03C3}"); // Σ
        assert_eq!(normalize_ci_from_normalized_cs("\u{03C3}"), "\u{03C3}"); // σ
        assert_eq!(normalize_ci_from_normalized_cs("\u{03C2}"), "\u{03C3}"); // ς
        // Lunate sigma ϲ/Ϲ fold to ϲ, not σ.
        assert_eq!(normalize_ci_from_normalized_cs("\u{03F2}"), "\u{03F2}"); // ϲ
        assert_eq!(normalize_ci_from_normalized_cs("\u{03F9}"), "\u{03F2}"); // Ϲ
    }

    #[test]
    fn ci_from_cs_hello() {
        assert_eq!(normalize_ci_from_normalized_cs("Hello.txt"), "hello.txt");
        assert_eq!(normalize_ci_from_normalized_cs("hello.txt"), "hello.txt");
        assert_eq!(normalize_ci_from_normalized_cs("HELLO.TXT"), "hello.txt");
    }

    #[test]
    fn ci_from_cs_nfc_nfd_equivalent() {
        // Both É (U+00C9) and E+\u{0301} CS-normalize to É (U+00C9).
        assert_eq!(
            normalize_ci_from_normalized_cs("\u{00C9}.txt"),
            "\u{00E9}.txt"
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

    #[test]
    fn normalize_space_vs_nbsp_distinct() {
        // Regular space and non-breaking space produce different normalized forms.
        let space = normalize_cs("a b").unwrap();
        let nbsp = normalize_cs("a\u{00A0}b").unwrap();
        assert_ne!(space, nbsp);
    }

    // --- Zero Width Joiner / Non-Joiner ---

    #[test]
    fn normalize_cs_preserves_zwj() {
        let result = normalize_cs("a\u{200D}b").unwrap();
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "a\u{200D}b");
    }

    #[test]
    fn normalize_cs_preserves_zwnj() {
        let result = normalize_cs("a\u{200C}b").unwrap();
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "a\u{200C}b");
    }

    #[test]
    fn ci_from_cs_preserves_zwj() {
        let result = normalize_ci_from_normalized_cs("a\u{200D}b");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "a\u{200D}b");
    }

    #[test]
    fn ci_from_cs_preserves_zwnj() {
        let result = normalize_ci_from_normalized_cs("a\u{200C}b");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "a\u{200C}b");
    }

    #[test]
    fn ci_from_cs_zwj_between_i_and_dot() {
        // ZWJ is a starter (CCC=0), so it blocks dot stripping.
        assert_eq!(
            normalize_ci_from_normalized_cs("i\u{200D}\u{0307}"),
            "i\u{200D}\u{0307}"
        );
    }

    #[test]
    fn ci_from_cs_zwnj_between_i_and_dot() {
        // ZWNJ is a starter (CCC=0), so it blocks dot stripping.
        assert_eq!(
            normalize_ci_from_normalized_cs("i\u{200C}\u{0307}"),
            "i\u{200C}\u{0307}"
        );
    }
}
