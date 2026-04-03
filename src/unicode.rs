use alloc::borrow::Cow;

use icu_casemap::CaseMapper;
use icu_normalizer::{ComposingNormalizer, DecomposingNormalizer};
use icu_properties::props::{CanonicalCombiningClass, GeneralCategory, SoftDotted, WhiteSpace};
use icu_properties::{CodePointMapData, CodePointSetData};

/// NFD normalization (canonical decomposition).
#[must_use]
pub fn nfd(s: &str) -> Cow<'_, str> {
    DecomposingNormalizer::new_nfd().normalize(s)
}

/// NFC normalization (canonical composition).
#[must_use]
pub fn nfc(s: &str) -> Cow<'_, str> {
    ComposingNormalizer::new_nfc().normalize(s)
}

/// Unicode `toCasefold()`.
#[must_use]
pub fn case_fold(s: &str) -> Cow<'_, str> {
    CaseMapper::new().fold_string(s)
}

/// Unicode `White_Space` property check.
#[must_use]
pub fn is_whitespace(c: char) -> bool {
    CodePointSetData::new::<WhiteSpace>().contains(c)
}

/// Whether `c` has Canonical Combining Class 0 (a "starter" — not a combining mark).
#[must_use]
pub fn is_starter(c: char) -> bool {
    CodePointMapData::<CanonicalCombiningClass>::new().get(c)
        == CanonicalCombiningClass::NotReordered
}

/// Whether `c` has Canonical Combining Class 230 (Above).
#[must_use]
pub fn is_above(c: char) -> bool {
    CodePointMapData::<CanonicalCombiningClass>::new().get(c) == CanonicalCombiningClass::Above
}

/// Whether `c` has the Unicode `Soft_Dotted` property (e.g. i, j, Cyrillic і, ј).
#[must_use]
pub fn is_soft_dotted(c: char) -> bool {
    CodePointSetData::new::<SoftDotted>().contains(c)
}

/// Whether `c` is an assigned character (not `GeneralCategory::Unassigned`).
#[must_use]
pub fn is_assigned(c: char) -> bool {
    CodePointMapData::<GeneralCategory>::new().get(c) != GeneralCategory::Unassigned
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;

    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{
        case_fold, is_above, is_assigned, is_soft_dotted, is_starter, is_whitespace, nfc, nfd,
    };

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

    #[test]
    fn case_fold_ipa_caseless() {
        // IPA characters are caseless — toCasefold() leaves them unchanged.
        let result = case_fold("\u{0250}\u{026A}\u{0283}"); // ɐ ɪ ʃ
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn case_fold_cherokee_folds_to_uppercase() {
        // Cherokee lowercase was added after uppercase, so toCasefold() maps
        // lowercase to uppercase for cross-version stability.
        assert_eq!(case_fold("\u{AB70}"), "\u{13A0}"); // ꭰ → Ꭰ
        assert_eq!(case_fold("\u{13A0}"), "\u{13A0}"); // Ꭰ → Ꭰ (unchanged)
    }

    #[test]
    fn case_fold_preserves_soft_dotted() {
        // Every Soft_Dotted character must fold to itself or to another
        // Soft_Dotted character. fixup_case_fold relies on this invariant.
        for cp in 0..=0x10_FFFFu32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            if !is_soft_dotted(c) {
                continue;
            }
            let s = alloc::string::String::from(c);
            let folded = case_fold(&s);
            let folded_chars: alloc::vec::Vec<char> = folded.chars().collect();
            assert_eq!(
                folded_chars.len(),
                1,
                "Soft_Dotted U+{cp:04X} folds to multiple characters: {folded:?}"
            );
            assert!(
                is_soft_dotted(folded_chars[0]),
                "Soft_Dotted U+{cp:04X} folds to U+{:04X} which is not Soft_Dotted",
                folded_chars[0] as u32
            );
        }
    }

    #[test]
    fn soft_dotted_exhaustive() {
        // Complete list of Soft_Dotted characters in Unicode 17.0.0.
        // Source: https://www.unicode.org/Public/17.0.0/ucd/PropList.txt
        #[rustfmt::skip]
        const SOFT_DOTTED: &[u32] = &[
            0x0069, 0x006A,             // LATIN SMALL LETTER I..J
            0x012F,                     // LATIN SMALL LETTER I WITH OGONEK
            0x0249,                     // LATIN SMALL LETTER J WITH STROKE
            0x0268,                     // LATIN SMALL LETTER I WITH STROKE
            0x029D,                     // LATIN SMALL LETTER J WITH CROSSED-TAIL
            0x02B2,                     // MODIFIER LETTER SMALL J
            0x03F3,                     // GREEK LETTER YOT
            0x0456,                     // CYRILLIC SMALL LETTER BYELORUSSIAN-UKRAINIAN I
            0x0458,                     // CYRILLIC SMALL LETTER JE
            0x1D62,                     // LATIN SUBSCRIPT SMALL LETTER I
            0x1D96,                     // LATIN SMALL LETTER I WITH RETROFLEX HOOK
            0x1DA4,                     // MODIFIER LETTER SMALL I WITH STROKE
            0x1DA8,                     // MODIFIER LETTER SMALL J WITH CROSSED-TAIL
            0x1E2D,                     // LATIN SMALL LETTER I WITH TILDE BELOW
            0x1ECB,                     // LATIN SMALL LETTER I WITH DOT BELOW
            0x2071,                     // SUPERSCRIPT LATIN SMALL LETTER I
            0x2148, 0x2149,             // DOUBLE-STRUCK ITALIC SMALL I..J
            0x2C7C,                     // LATIN SUBSCRIPT SMALL LETTER J
            0x1D422, 0x1D423,           // MATHEMATICAL BOLD SMALL I..J
            0x1D456, 0x1D457,           // MATHEMATICAL ITALIC SMALL I..J
            0x1D48A, 0x1D48B,           // MATHEMATICAL BOLD ITALIC SMALL I..J
            0x1D4BE, 0x1D4BF,           // MATHEMATICAL SCRIPT SMALL I..J
            0x1D4F2, 0x1D4F3,           // MATHEMATICAL BOLD SCRIPT SMALL I..J
            0x1D526, 0x1D527,           // MATHEMATICAL FRAKTUR SMALL I..J
            0x1D55A, 0x1D55B,           // MATHEMATICAL DOUBLE-STRUCK SMALL I..J
            0x1D58E, 0x1D58F,           // MATHEMATICAL BOLD FRAKTUR SMALL I..J
            0x1D5C2, 0x1D5C3,           // MATHEMATICAL SANS-SERIF SMALL I..J
            0x1D5F6, 0x1D5F7,           // MATHEMATICAL SANS-SERIF BOLD SMALL I..J
            0x1D62A, 0x1D62B,           // MATHEMATICAL SANS-SERIF ITALIC SMALL I..J
            0x1D65E, 0x1D65F,           // MATHEMATICAL SANS-SERIF BOLD ITALIC SMALL I..J
            0x1D692, 0x1D693,           // MATHEMATICAL MONOSPACE SMALL I..J
            0x1DF1A,                    // LATIN SMALL LETTER I WITH STROKE AND RETROFLEX HOOK
            0x1E04C, 0x1E04D,           // MODIFIER LETTER CYRILLIC SMALL BYELORUSSIAN-UKRAINIAN I..JE
            0x1E068,                    // CYRILLIC SUBSCRIPT SMALL LETTER BYELORUSSIAN-UKRAINIAN I
        ];
        let mut found = alloc::vec::Vec::new();
        for cp in 0..=0x10_FFFFu32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            if is_soft_dotted(c) {
                found.push(cp);
            }
        }
        assert_eq!(found, SOFT_DOTTED);
    }

    #[test]
    fn above_exhaustive() {
        // Complete list of CCC=230 (Above) character ranges in Unicode 17.0.0.
        // Source: https://www.unicode.org/Public/17.0.0/ucd/UnicodeData.txt
        #[rustfmt::skip]
        const ABOVE_RANGES: &[core::ops::RangeInclusive<u32>] = &[
            0x0300..=0x0314, 0x033D..=0x0344, 0x0346..=0x0346,
            0x034A..=0x034C, 0x0350..=0x0352, 0x0357..=0x0357,
            0x035B..=0x035B, 0x0363..=0x036F, 0x0483..=0x0487,
            0x0592..=0x0595, 0x0597..=0x0599, 0x059C..=0x05A1,
            0x05A8..=0x05A9, 0x05AB..=0x05AC, 0x05AF..=0x05AF,
            0x05C4..=0x05C4, 0x0610..=0x0617, 0x0653..=0x0654,
            0x0657..=0x065B, 0x065D..=0x065E, 0x06D6..=0x06DC,
            0x06DF..=0x06E2, 0x06E4..=0x06E4, 0x06E7..=0x06E8,
            0x06EB..=0x06EC, 0x0730..=0x0730, 0x0732..=0x0733,
            0x0735..=0x0736, 0x073A..=0x073A, 0x073D..=0x073D,
            0x073F..=0x0741, 0x0743..=0x0743, 0x0745..=0x0745,
            0x0747..=0x0747, 0x0749..=0x074A, 0x07EB..=0x07F1,
            0x07F3..=0x07F3, 0x0816..=0x0819, 0x081B..=0x0823,
            0x0825..=0x0827, 0x0829..=0x082D, 0x0897..=0x0898,
            0x089C..=0x089F, 0x08CA..=0x08CE, 0x08D4..=0x08E1,
            0x08E4..=0x08E5, 0x08E7..=0x08E8, 0x08EA..=0x08EC,
            0x08F3..=0x08F5, 0x08F7..=0x08F8, 0x08FB..=0x08FF,
            0x0951..=0x0951, 0x0953..=0x0954, 0x09FE..=0x09FE,
            0x0F82..=0x0F83, 0x0F86..=0x0F87, 0x135D..=0x135F,
            0x17DD..=0x17DD, 0x193A..=0x193A, 0x1A17..=0x1A17,
            0x1A75..=0x1A7C, 0x1AB0..=0x1AB4, 0x1ABB..=0x1ABC,
            0x1AC1..=0x1AC2, 0x1AC5..=0x1AC9, 0x1ACB..=0x1ADC,
            0x1AE0..=0x1AE5, 0x1AE7..=0x1AEA, 0x1B6B..=0x1B6B,
            0x1B6D..=0x1B73, 0x1CD0..=0x1CD2, 0x1CDA..=0x1CDB,
            0x1CE0..=0x1CE0, 0x1CF4..=0x1CF4, 0x1CF8..=0x1CF9,
            0x1DC0..=0x1DC1, 0x1DC3..=0x1DC9, 0x1DCB..=0x1DCC,
            0x1DD1..=0x1DF5, 0x1DFB..=0x1DFB, 0x1DFE..=0x1DFE,
            0x20D0..=0x20D1, 0x20D4..=0x20D7, 0x20DB..=0x20DC,
            0x20E1..=0x20E1, 0x20E7..=0x20E7, 0x20E9..=0x20E9,
            0x20F0..=0x20F0, 0x2CEF..=0x2CF1, 0x2DE0..=0x2DFF,
            0xA66F..=0xA66F, 0xA674..=0xA67D, 0xA69E..=0xA69F,
            0xA6F0..=0xA6F1, 0xA8E0..=0xA8F1, 0xAAB0..=0xAAB0,
            0xAAB2..=0xAAB3, 0xAAB7..=0xAAB8, 0xAABE..=0xAABF,
            0xAAC1..=0xAAC1, 0xFE20..=0xFE26, 0xFE2E..=0xFE2F,
            0x10376..=0x1037A, 0x10A0F..=0x10A0F, 0x10A38..=0x10A38,
            0x10AE5..=0x10AE5, 0x10D24..=0x10D27, 0x10D69..=0x10D6D,
            0x10EAB..=0x10EAC, 0x10F48..=0x10F4A, 0x10F4C..=0x10F4C,
            0x10F82..=0x10F82, 0x10F84..=0x10F84, 0x11100..=0x11102,
            0x11366..=0x1136C, 0x11370..=0x11374, 0x1145E..=0x1145E,
            0x16B30..=0x16B36, 0x1D185..=0x1D189, 0x1D1AA..=0x1D1AD,
            0x1D242..=0x1D244, 0x1E000..=0x1E006, 0x1E008..=0x1E018,
            0x1E01B..=0x1E021, 0x1E023..=0x1E024, 0x1E026..=0x1E02A,
            0x1E08F..=0x1E08F, 0x1E130..=0x1E136, 0x1E2AE..=0x1E2AE,
            0x1E2EC..=0x1E2EF, 0x1E4EF..=0x1E4EF, 0x1E5EE..=0x1E5EE,
            0x1E6E3..=0x1E6E3, 0x1E6E6..=0x1E6E6, 0x1E6EE..=0x1E6EF,
            0x1E6F5..=0x1E6F5, 0x1E944..=0x1E949,
        ];
        let mut expected = alloc::vec::Vec::new();
        for range in ABOVE_RANGES {
            for cp in range.clone() {
                expected.push(cp);
            }
        }
        let mut found = alloc::vec::Vec::new();
        for cp in 0..=0x10_FFFFu32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            if is_above(c) {
                found.push(cp);
            }
        }
        assert_eq!(found, expected);
    }

    #[test]
    fn white_space_exhaustive() {
        // Complete list of White_Space characters in Unicode 17.0.0.
        // Source: https://www.unicode.org/Public/17.0.0/ucd/PropList.txt
        #[rustfmt::skip]
        const WHITE_SPACE: &[u32] = &[
            0x0009, 0x000A, 0x000B, 0x000C, 0x000D, // <control-0009>..<control-000D>
            0x0020,                     // SPACE
            0x0085,                     // <control-0085>
            0x00A0,                     // NO-BREAK SPACE
            0x1680,                     // OGHAM SPACE MARK
            0x2000, 0x2001, 0x2002, 0x2003, 0x2004, // EN QUAD..FOUR-PER-EM SPACE
            0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200A, // SIX-PER-EM SPACE..HAIR SPACE
            0x2028,                     // LINE SEPARATOR
            0x2029,                     // PARAGRAPH SEPARATOR
            0x202F,                     // NARROW NO-BREAK SPACE
            0x205F,                     // MEDIUM MATHEMATICAL SPACE
            0x3000,                     // IDEOGRAPHIC SPACE
        ];
        let mut found = alloc::vec::Vec::new();
        for cp in 0..=0x10_FFFFu32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            if is_whitespace(c) {
                found.push(cp);
            }
        }
        assert_eq!(found, WHITE_SPACE);
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

    // --- is_above ---

    #[test]
    fn is_above_combining_acute() {
        assert!(is_above('\u{0301}')); // CCC=230
    }

    #[test]
    fn is_above_combining_dot_above() {
        assert!(is_above('\u{0307}')); // CCC=230
    }

    #[test]
    fn is_above_combining_cedilla() {
        assert!(!is_above('\u{0327}')); // CCC=202
    }

    #[test]
    fn is_above_ascii_letter() {
        assert!(!is_above('a')); // CCC=0
    }

    // --- is_soft_dotted ---

    #[test]
    fn is_soft_dotted_latin_i() {
        assert!(is_soft_dotted('i'));
    }

    #[test]
    fn is_soft_dotted_latin_j() {
        assert!(is_soft_dotted('j'));
    }

    #[test]
    fn is_soft_dotted_cyrillic_i() {
        assert!(is_soft_dotted('\u{0456}')); // CYRILLIC SMALL LETTER BYELORUSSIAN-UKRAINIAN I
    }

    #[test]
    fn is_soft_dotted_cyrillic_je() {
        assert!(is_soft_dotted('\u{0458}')); // CYRILLIC SMALL LETTER JE
    }

    #[test]
    fn is_soft_dotted_latin_i_ogonek() {
        assert!(is_soft_dotted('\u{012F}')); // LATIN SMALL LETTER I WITH OGONEK
    }

    #[test]
    fn is_soft_dotted_uppercase_i() {
        assert!(!is_soft_dotted('I'));
    }

    #[test]
    fn is_soft_dotted_uppercase_j() {
        assert!(!is_soft_dotted('J'));
    }

    #[test]
    fn is_soft_dotted_math_sans_bold_i() {
        assert!(is_soft_dotted('\u{1D5F6}')); // MATHEMATICAL SANS-SERIF BOLD SMALL I
    }

    #[test]
    fn is_soft_dotted_greek_yot() {
        assert!(is_soft_dotted('\u{03F3}')); // GREEK LETTER YOT
    }

    #[test]
    fn is_soft_dotted_latin_i_stroke() {
        assert!(is_soft_dotted('\u{0268}')); // LATIN SMALL LETTER I WITH STROKE
    }

    #[test]
    fn is_soft_dotted_dotless_i() {
        assert!(!is_soft_dotted('\u{0131}')); // ı is NOT Soft_Dotted
    }

    #[test]
    fn is_soft_dotted_dotless_j() {
        assert!(!is_soft_dotted('\u{0237}')); // ȷ is NOT Soft_Dotted
    }

    // --- is_assigned ---

    #[test]
    fn is_assigned_ascii() {
        assert!(is_assigned('a'));
        assert!(is_assigned('0'));
        assert!(is_assigned(' '));
    }

    #[test]
    fn is_assigned_cjk() {
        assert!(is_assigned('日'));
    }

    #[test]
    fn is_assigned_emoji() {
        assert!(is_assigned('\u{1F600}')); // GRINNING FACE
    }

    #[test]
    fn is_assigned_unassigned() {
        assert!(!is_assigned('\u{0378}')); // unassigned in Greek block
    }

    #[test]
    fn is_assigned_pua() {
        // Private Use Area characters are assigned (General_Category=Private_Use).
        assert!(is_assigned('\u{E000}'));
    }

    #[test]
    fn is_assigned_noncharacter() {
        // Noncharacters are permanently reserved and must not appear in interchange.
        // They have General_Category=Cn (Unassigned) and we treat them as such.
        assert!(!is_assigned('\u{FDD0}'));
        assert!(!is_assigned('\u{FFFE}'));
        assert!(!is_assigned('\u{10FFFF}'));
    }

    #[test]
    fn is_assigned_unicode17_saudi_riyal() {
        assert!(is_assigned('\u{20C1}')); // SAUDI RIYAL SIGN (Unicode 17.0.0)
    }

    #[test]
    fn is_assigned_emoji_boundary() {
        assert!(is_assigned('\u{1FA7C}')); // CRUTCH (Unicode 14.0)
        assert!(!is_assigned('\u{1FA7D}')); // unassigned
        assert!(is_assigned('\u{1FAEA}')); // DISTORTED FACE (Unicode 17.0.0)
        assert!(!is_assigned('\u{1FAEB}')); // unassigned
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
