use alloc::borrow::Cow;

use icu_casemap::CaseMapper;
use icu_normalizer::{ComposingNormalizer, DecomposingNormalizer};
use icu_properties::props::{CanonicalCombiningClass, WhiteSpace};
use icu_properties::{CodePointMapData, CodePointSetData};

/// NFD normalization (canonical decomposition).
pub fn nfd(s: &str) -> Cow<'_, str> {
    DecomposingNormalizer::new_nfd().normalize(s)
}

/// NFC normalization (canonical composition).
pub fn nfc(s: &str) -> Cow<'_, str> {
    ComposingNormalizer::new_nfc().normalize(s)
}

/// Unicode `toCasefold()`.
pub fn case_fold(s: &str) -> Cow<'_, str> {
    CaseMapper::new().fold_string(s)
}

/// Unicode `White_Space` property check.
pub fn is_whitespace(c: char) -> bool {
    CodePointSetData::new::<WhiteSpace>().contains(c)
}

/// Whether `c` has Canonical Combining Class 0 (a "starter" — not a combining mark).
pub fn is_starter(c: char) -> bool {
    CodePointMapData::<CanonicalCombiningClass>::new().get(c)
        == CanonicalCombiningClass::NotReordered
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{case_fold, is_starter, is_whitespace, nfc, nfd};

    // --- nfd / nfc ---

    #[test]
    fn nfd_decomposes() {
        assert_eq!(nfd("\u{00E9}"), "e\u{0301}");
    }

    #[test]
    fn nfc_composes() {
        assert_eq!(nfc("e\u{0301}"), "\u{00E9}");
    }

    #[test]
    fn nfd_ascii_unchanged() {
        let result = nfd("hello");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn nfc_ascii_unchanged() {
        let result = nfc("hello");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn nfd_idempotent() {
        let s = "e\u{0301}\u{00F1}\u{00E9}";
        assert_eq!(nfd(&nfd(s)), nfd(s));
    }

    #[test]
    fn nfc_idempotent() {
        let s = "e\u{0301}\u{00F1}\u{00E9}";
        assert_eq!(nfc(&nfc(s)), nfc(s));
    }

    #[test]
    fn nfd_then_nfc_equals_nfc() {
        let s = "e\u{0301}\u{00F1}\u{00E9}";
        assert_eq!(nfc(&nfd(s)), nfc(s));
    }

    #[test]
    fn nfc_then_nfd_equals_nfd() {
        let s = "e\u{0301}\u{00F1}\u{00E9}";
        assert_eq!(nfd(&nfc(s)), nfd(s));
    }

    // --- case_fold ---

    #[test]
    fn case_fold_ascii_lowercase() {
        assert_eq!(case_fold("HELLO"), "hello");
    }

    #[test]
    fn case_fold_ascii_unchanged() {
        let result = case_fold("hello");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "hello");
    }

    #[test]
    fn case_fold_mixed_case() {
        assert_eq!(case_fold("HeLLo"), "hello");
    }

    #[test]
    fn case_fold_german_eszett() {
        assert_eq!(case_fold("\u{00DF}"), "ss");
    }

    #[test]
    fn case_fold_turkish_dotted_i() {
        let folded = case_fold("\u{0130}");
        assert_eq!(folded, "i\u{0307}");
    }

    #[test]
    fn case_fold_greek_sigma() {
        assert_eq!(case_fold("\u{03A3}"), "\u{03C3}");
        assert_eq!(case_fold("\u{03C2}"), "\u{03C3}");
    }

    // --- is_whitespace ---

    #[test]
    fn is_whitespace_space() {
        assert!(is_whitespace(' '));
    }

    #[test]
    fn is_whitespace_tab() {
        assert!(is_whitespace('\t'));
    }

    #[test]
    fn is_whitespace_newline() {
        assert!(is_whitespace('\n'));
    }

    #[test]
    fn is_whitespace_ideographic_space() {
        assert!(is_whitespace('\u{3000}'));
    }

    #[test]
    fn is_whitespace_no_break_space() {
        assert!(is_whitespace('\u{00A0}'));
    }

    #[test]
    fn is_whitespace_ascii_letter() {
        assert!(!is_whitespace('a'));
    }

    #[test]
    fn is_whitespace_digit() {
        assert!(!is_whitespace('0'));
    }

    #[test]
    fn is_whitespace_combining_mark() {
        assert!(!is_whitespace('\u{0301}'));
    }

    // --- is_starter ---

    #[test]
    fn is_starter_ascii_letter() {
        assert!(is_starter('a'));
    }

    #[test]
    fn is_starter_digit() {
        assert!(is_starter('0'));
    }

    #[test]
    fn is_starter_space() {
        assert!(is_starter(' '));
    }

    #[test]
    fn is_starter_combining_acute() {
        assert!(!is_starter('\u{0301}'));
    }

    #[test]
    fn is_starter_combining_cedilla() {
        assert!(!is_starter('\u{0327}'));
    }

    #[test]
    fn is_starter_combining_dot_above() {
        assert!(!is_starter('\u{0307}'));
    }

    #[test]
    fn is_starter_cjk() {
        assert!(is_starter('日'));
    }

    // --- Non-standard characters (unassigned, PUA, noncharacters) ---

    // U+0378: unassigned in Greek block
    // U+E000: Private Use Area
    // U+FDD0: noncharacter
    // U+FFFE: noncharacter
    // U+FFFF: noncharacter
    // U+10FFFF: noncharacter (last valid code point)

    #[test]
    fn nfd_unassigned_unchanged() {
        let result = nfd("\u{0378}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{0378}");
    }

    #[test]
    fn nfd_pua_unchanged() {
        let result = nfd("\u{E000}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{E000}");
    }

    #[test]
    fn nfd_noncharacter_unchanged() {
        for c in ['\u{FDD0}', '\u{FFFE}', '\u{FFFF}', '\u{10FFFF}'] {
            let s = alloc::string::String::from(c);
            let result = nfd(&s);
            assert!(matches!(result, Cow::Borrowed(_)), "NFD changed {c:?}");
            assert_eq!(result.chars().next(), Some(c));
        }
    }

    #[test]
    fn nfc_unassigned_unchanged() {
        let result = nfc("\u{0378}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{0378}");
    }

    #[test]
    fn nfc_pua_unchanged() {
        let result = nfc("\u{E000}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{E000}");
    }

    #[test]
    fn nfc_noncharacter_unchanged() {
        for c in ['\u{FDD0}', '\u{FFFE}', '\u{FFFF}', '\u{10FFFF}'] {
            let s = alloc::string::String::from(c);
            let result = nfc(&s);
            assert!(matches!(result, Cow::Borrowed(_)), "NFC changed {c:?}");
            assert_eq!(result.chars().next(), Some(c));
        }
    }

    #[test]
    fn case_fold_unassigned_unchanged() {
        let result = case_fold("\u{0378}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{0378}");
    }

    #[test]
    fn case_fold_pua_unchanged() {
        let result = case_fold("\u{E000}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{E000}");
    }

    #[test]
    fn case_fold_noncharacter_unchanged() {
        for c in ['\u{FDD0}', '\u{FFFE}', '\u{FFFF}', '\u{10FFFF}'] {
            let s = alloc::string::String::from(c);
            let result = case_fold(&s);
            assert!(
                matches!(result, Cow::Borrowed(_)),
                "case_fold changed {c:?}"
            );
            assert_eq!(result.chars().next(), Some(c));
        }
    }

    #[test]
    fn is_whitespace_unassigned() {
        assert!(!is_whitespace('\u{0378}'));
    }

    #[test]
    fn is_whitespace_pua() {
        assert!(!is_whitespace('\u{E000}'));
    }

    #[test]
    fn is_whitespace_noncharacter() {
        assert!(!is_whitespace('\u{FDD0}'));
        assert!(!is_whitespace('\u{FFFE}'));
        assert!(!is_whitespace('\u{FFFF}'));
        assert!(!is_whitespace('\u{10FFFF}'));
    }

    #[test]
    fn is_starter_unassigned() {
        // Unassigned code points have CCC=0, so they are starters.
        assert!(is_starter('\u{0378}'));
    }

    #[test]
    fn is_starter_pua() {
        // PUA code points have CCC=0, so they are starters.
        assert!(is_starter('\u{E000}'));
    }

    #[test]
    fn is_starter_noncharacter() {
        // Noncharacters have CCC=0, so they are starters.
        assert!(is_starter('\u{FDD0}'));
        assert!(is_starter('\u{FFFE}'));
        assert!(is_starter('\u{FFFF}'));
        assert!(is_starter('\u{10FFFF}'));
    }

    // --- Supplementary plane characters (outside BMP, above U+FFFF) ---

    // Case folding: Deseret script has upper/lower pairs outside BMP.
    #[test]
    fn case_fold_deseret() {
        // U+10400 DESERET CAPITAL LETTER LONG I → U+10428 DESERET SMALL LETTER LONG I
        assert_eq!(case_fold("\u{10400}"), "\u{10428}");
    }

    // Case folding: Mathematical Alphanumeric Symbols are NOT folded by toCasefold()
    // (they have no CaseFolding.txt mapping). They remain unchanged.
    #[test]
    fn case_fold_mathematical_bold_unchanged() {
        let result = case_fold("\u{1D400}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{1D400}");
    }

    // NFD: CJK Compatibility Ideographs Supplement decompose to unified ideographs.
    #[test]
    fn nfd_cjk_compat_supplement() {
        // U+2F800 CJK COMPATIBILITY IDEOGRAPH-2F800 → U+4E3D
        assert_eq!(nfd("\u{2F800}"), "\u{4E3D}");
    }

    // NFC also decomposes CJK compatibility ideographs (they have canonical decompositions).
    #[test]
    fn nfc_cjk_compat_supplement() {
        assert_eq!(nfc("\u{2F800}"), "\u{4E3D}");
    }

    // NFD: Musical symbols with canonical decompositions.
    #[test]
    fn nfd_musical_symbol() {
        // U+1D15E MUSICAL SYMBOL HALF NOTE → U+1D157 VOID NOTEHEAD + U+1D165 COMBINING STEM
        assert_eq!(nfd("\u{1D15E}"), "\u{1D157}\u{1D165}");
    }

    // NFC does NOT recompose musical symbols: U+1D15E is a composition exclusion.
    #[test]
    fn nfc_musical_symbol_composition_exclusion() {
        // The decomposed form stays decomposed under NFC.
        assert_eq!(nfc("\u{1D157}\u{1D165}"), "\u{1D157}\u{1D165}");
    }

    // Non-starters outside BMP: musical combining marks.
    #[test]
    fn is_starter_musical_combining_stem() {
        // U+1D165 MUSICAL SYMBOL COMBINING STEM (CCC=216)
        assert!(!is_starter('\u{1D165}'));
    }

    #[test]
    fn is_starter_musical_combining_augmentation_dot() {
        // U+1D16D MUSICAL SYMBOL COMBINING AUGMENTATION DOT (CCC=226)
        assert!(!is_starter('\u{1D16D}'));
    }

    // Starters outside BMP: emoji, CJK Extension B, Deseret letters.
    #[test]
    fn is_starter_supplementary() {
        assert!(is_starter('\u{1F600}')); // GRINNING FACE
        assert!(is_starter('\u{20000}')); // CJK UNIFIED IDEOGRAPH-20000
        assert!(is_starter('\u{10400}')); // DESERET CAPITAL LETTER LONG I
    }

    // No whitespace exists outside BMP.
    #[test]
    fn is_whitespace_supplementary() {
        assert!(!is_whitespace('\u{1F600}')); // emoji
        assert!(!is_whitespace('\u{20000}')); // CJK Extension B
        assert!(!is_whitespace('\u{10400}')); // Deseret
    }

    // NFD/NFC: supplementary characters without decompositions are unchanged.
    #[test]
    fn nfd_emoji_unchanged() {
        let result = nfd("\u{1F600}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{1F600}");
    }

    #[test]
    fn nfc_emoji_unchanged() {
        let result = nfc("\u{1F600}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{1F600}");
    }

    // Case fold: supplementary characters without case mappings are unchanged.
    #[test]
    fn case_fold_emoji_unchanged() {
        let result = case_fold("\u{1F600}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{1F600}");
    }

    // --- Unicode 17.0.0 characters ---

    // U+20C1 SAUDI RIYAL SIGN (new currency symbol)
    #[test]
    fn nfd_saudi_riyal_unchanged() {
        let result = nfd("\u{20C1}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{20C1}");
    }

    #[test]
    fn nfc_saudi_riyal_unchanged() {
        let result = nfc("\u{20C1}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{20C1}");
    }

    #[test]
    fn case_fold_saudi_riyal_unchanged() {
        let result = case_fold("\u{20C1}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{20C1}");
    }

    #[test]
    fn is_whitespace_saudi_riyal() {
        assert!(!is_whitespace('\u{20C1}'));
    }

    #[test]
    fn is_starter_saudi_riyal() {
        assert!(is_starter('\u{20C1}'));
    }

    // U+2B96 EQUALS SIGN WITH INFINITY ABOVE (new math symbol)
    #[test]
    fn nfd_equals_infinity_unchanged() {
        let result = nfd("\u{2B96}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{2B96}");
    }

    #[test]
    fn case_fold_equals_infinity_unchanged() {
        let result = case_fold("\u{2B96}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{2B96}");
    }

    #[test]
    fn is_starter_equals_infinity() {
        assert!(is_starter('\u{2B96}'));
    }

    // Tolong Siki script (U+11DB0..U+11DEF)
    #[test]
    fn nfd_tolong_siki_unchanged() {
        let result = nfd("\u{11DB0}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{11DB0}");
    }

    #[test]
    fn is_starter_tolong_siki() {
        assert!(is_starter('\u{11DB0}'));
    }

    #[test]
    fn is_whitespace_tolong_siki() {
        assert!(!is_whitespace('\u{11DB0}'));
    }

    // Beria Erfe script (U+16EA0..U+16EDF)
    #[test]
    fn nfd_beria_erfe_unchanged() {
        let result = nfd("\u{16EA0}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{16EA0}");
    }

    #[test]
    fn is_starter_beria_erfe() {
        assert!(is_starter('\u{16EA0}'));
    }

    // CJK Unified Ideographs Extension J (U+323B0..U+3347F)
    #[test]
    fn nfd_cjk_extension_j_unchanged() {
        let result = nfd("\u{323B0}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{323B0}");
    }

    #[test]
    fn nfc_cjk_extension_j_unchanged() {
        let result = nfc("\u{323B0}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{323B0}");
    }

    #[test]
    fn case_fold_cjk_extension_j_unchanged() {
        let result = case_fold("\u{323B0}");
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, "\u{323B0}");
    }

    #[test]
    fn is_starter_cjk_extension_j() {
        assert!(is_starter('\u{323B0}'));
    }
}
