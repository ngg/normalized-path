#![no_main]

use icu_casemap::options::{LeadingAdjustment, TitlecaseOptions, TrailingCase};
use icu_casemap::{CaseMapper, TitlecaseMapper};
use icu_locale_core::langid;
use libfuzzer_sys::fuzz_target;
#[cfg(target_vendor = "apple")]
use normalized_path::test_helpers::apple_compatible_from_normalized_cs_fallback;
use normalized_path::test_helpers::{
    apple_compatible_from_normalized_cs, case_fold, fixup_case_fold, is_reserved_on_windows,
    map_control_chars, map_fullwidth, nfc, nfd, normalize_ci_from_normalized_cs, normalize_cs,
    trim_whitespace_like, validate_path_element, windows_compatible_from_normalized_cs,
};
use normalized_path::{CaseSensitivity, PathElement};

/// Reverse of `map_fullwidth`: map ASCII printable characters to their fullwidth equivalents.
fn unmap_fullwidth(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '!'..='~' => char::from_u32(c as u32 + 0xFEE0).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// Reverse of `map_control_chars`: map Control Pictures back to control characters.
fn unmap_control_chars(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{2401}'..='\u{241F}' => char::from_u32(c as u32 - 0x2400).unwrap_or(c),
            '\u{2421}' => '\x7F',
            _ => c,
        })
        .collect()
}

fn fuzz_normalize(data: &[u8], cs: CaseSensitivity) {
    #[cfg(target_vendor = "apple")]
    {
        let decoded = String::from_utf8_lossy(data);
        if !decoded.contains('\0') {
            assert!(
                apple_compatible_from_normalized_cs(&decoded).is_ok(),
                "apple_compatible_from_normalized_cs failed\n\
                 decoded: {decoded:?}"
            );
        }
    }

    // Construct via from_bytes — also exercises the UTF-8 rejection path.
    let pe = match PathElement::from_bytes(data, cs) {
        Ok(pe) => pe,
        Err(err) => {
            #[cfg(target_vendor = "apple")]
            assert_ne!(
                *err.kind(),
                normalized_path::ErrorKind::GetFileSystemRepresentationError,
                "PathElement construction failed with GetFileSystemRepresentationError\n\
                 data: {data:?}"
            );
            let _ = err;
            return;
        }
    };
    let input = pe.original();
    let normalized = pe.normalized();
    validate_path_element(input).expect("validate_path_element must accept original");
    validate_path_element(normalized).expect("validate_path_element must accept normalized output");
    validate_path_element(pe.os_compatible())
        .expect("validate_path_element must accept os_compatible output");

    // from_utf8_lossy of the raw data must produce the same original.
    let decoded = String::from_utf8_lossy(data);
    assert_eq!(
        input, &*decoded,
        "from_bytes original does not match from_utf8_lossy\n\
         data:    {data:?}\n\
         original: {input:?}\n\
         decoded:  {decoded:?}"
    );

    // Constructing from the decoded string must produce the same normalized form.
    let pe_decoded = PathElement::new(&*decoded, cs)
        .expect("assertion error: PathElement::new failed on from_utf8_lossy output");
    assert_eq!(
        normalized,
        pe_decoded.normalized(),
        "normalize mismatch between from_bytes and new(from_utf8_lossy(data))\n\
         data:    {data:?}\n\
         from_bytes normalized: {normalized:?}\n\
         new normalized:        {:?}",
        pe_decoded.normalized()
    );

    // If the data is valid UTF-8, from_bytes and new must agree exactly.
    if let Ok(s) = core::str::from_utf8(data) {
        assert_eq!(
            input, s,
            "from_bytes original differs from raw UTF-8 input\n\
             data: {data:?}"
        );
        let pe_str = PathElement::new(s, cs)
            .expect("assertion error: PathElement::new failed on valid UTF-8 input");
        assert_eq!(
            normalized,
            pe_str.normalized(),
            "normalize mismatch between from_bytes and new on valid UTF-8\n\
             data: {data:?}"
        );
    }

    // is_normalized and is_os_compatible must agree with value comparison.
    assert_eq!(
        pe.is_normalized(),
        pe.original() == pe.normalized(),
        "is_normalized mismatch\n\
         original:   {:?}\n\
         normalized: {:?}",
        pe.original(),
        pe.normalized()
    );
    assert_eq!(
        pe.is_os_compatible(),
        pe.original() == pe.os_compatible(),
        "is_os_compatible mismatch\n\
         original:      {:?}\n\
         os_compatible: {:?}",
        pe.original(),
        pe.os_compatible()
    );

    // is_reserved_on_windows must be stable under NFD and NFD→casefold→fixup_case_fold→NFD.
    assert_eq!(
        is_reserved_on_windows(normalized),
        is_reserved_on_windows(&nfd(normalized)),
        "is_reserved_on_windows mismatch after nfd\n\
         normalized: {normalized:?}"
    );
    assert_eq!(
        is_reserved_on_windows(normalized),
        is_reserved_on_windows(&nfd(&fixup_case_fold(&case_fold(&nfd(normalized))))),
        "is_reserved_on_windows mismatch after nfd(fixup_case_fold(case_fold(nfd(...))))\n\
         normalized: {normalized:?}"
    );
    if is_reserved_on_windows(normalized) {
        assert!(
            normalized.chars().next().unwrap().is_ascii_alphabetic(),
            "reserved name starts with non-ASCII-letter\n\
             normalized: {normalized:?}"
        );
    }

    let check = |name: &str, transformed: &str| {
        let Ok(pe) = PathElement::new(transformed, cs) else {
            panic!(
                "normalize failed after {name}\n\
                 original input: {input:?}\n\
                 transformed:    {transformed:?}\n\
                 normalized:       {normalized:?}"
            );
        };
        assert_eq!(
            normalized,
            pe.normalized(),
            "normalize mismatch after {name}\n\
             original input: {input:?}\n\
             transformed:    {transformed:?}\n\
             normalized:       {normalized:?}\n\
             got:            {:?}",
            pe.normalized()
        );
    };

    let nfd_input = nfd(input);
    check("nfd", &nfd_input);
    check("nfc", &nfc(input));
    check("map_fullwidth", &map_fullwidth(input));
    check("map_control_chars", &map_control_chars(input));
    check("unmap_fullwidth", &unmap_fullwidth(input));
    check("unmap_control_chars", &unmap_control_chars(input));
    check(
        "windows_compatible_from_normalized_cs",
        &*windows_compatible_from_normalized_cs(input),
    );
    check(
        "apple_compatible_from_normalized_cs",
        &*apple_compatible_from_normalized_cs(input).unwrap(),
    );
    #[cfg(target_vendor = "apple")]
    check(
        "apple_compatible_from_normalized_cs_fallback",
        &*apple_compatible_from_normalized_cs_fallback(input),
    );

    let trimmed = input.trim();
    if normalize_cs(trimmed).is_ok() {
        check("trim", trimmed);
    }

    let trimmed_ws = trim_whitespace_like(input);
    if normalize_cs(trimmed_ws).is_ok() {
        check("trim_whitespace_like", trimmed_ws);
    }

    if cs == CaseSensitivity::Insensitive {
        // Post-case-fold fixup is only applied in CI mode.
        check("fixup_case_fold", &fixup_case_fold(input));

        // CS normalization is a subset of CI (everything except case folding
        // and Turkish İ mapping), so normalize_cs fed back into CI
        // should produce the same result.
        let cs_normalized = normalize_cs(input).unwrap();
        check("normalize_sensitive", &cs_normalized);

        // Applying NFD → case fold → Turkish İ mapping → NFC to a CS-normalized
        // input should produce the same result as CI normalization directly.
        let ci_from_cs = normalize_ci_from_normalized_cs(&cs_normalized);
        assert_eq!(
            normalized, ci_from_cs,
            "CI mismatch after normalize_ci_from_normalized_cs\n\
             original input: {input:?}\n\
             cs_normalized:  {cs_normalized:?}\n\
             ci_from_cs:     {ci_from_cs:?}\n\
             normalized:       {normalized:?}"
        );

        // CI-normalized output must already be CS-stable.
        assert_eq!(
            normalized,
            normalize_cs(normalized).unwrap(),
            "normalize_cs(normalize_ci(input)) != normalize_ci(input)\n\
             original input: {input:?}\n\
             normalized:     {normalized:?}"
        );

        check("ascii_lowercase", &input.to_ascii_lowercase());
        check("ascii_uppercase", &input.to_ascii_uppercase());

        // D145 requires NFD before case folding because U+0345 COMBINING GREEK
        // YPOGEGRAMMENI (CCC 240) case-folds to U+03B9 (a starter), changing
        // canonical ordering. Check the NFD form for U+0345 since precomposed
        // characters like U+1FC3 (ᾳ) contain it only after decomposition.
        let case_input = if nfd_input.contains('\u{0345}') {
            &*nfd_input
        } else {
            input
        };
        check("case_fold", &case_fold(case_input));
        let cm = CaseMapper::new();
        check("fold_turkic", &cm.fold_turkic_string(case_input));

        let tc = TitlecaseMapper::new();
        let check_locale = |langid: &icu_locale_core::LanguageIdentifier| {
            let tag = langid.to_string();
            check(
                &format!("icu_lowercase_{tag}"),
                &cm.lowercase_to_string(case_input, langid),
            );
            check(
                &format!("icu_uppercase_{tag}"),
                &cm.uppercase_to_string(case_input, langid),
            );
            for (trailing, trailing_name) in [
                (TrailingCase::Lower, "lower"),
                (TrailingCase::Unchanged, "unchanged"),
            ] {
                for (leading, leading_name) in [
                    (LeadingAdjustment::Auto, "auto"),
                    (LeadingAdjustment::None, "none"),
                    (LeadingAdjustment::ToCased, "cased"),
                ] {
                    let mut opts = TitlecaseOptions::default();
                    opts.trailing_case = Some(trailing);
                    opts.leading_adjustment = Some(leading);
                    check(
                        &format!("icu_titlecase_{tag}_{trailing_name}_{leading_name}"),
                        &tc.titlecase_segment_to_string(case_input, langid, opts),
                    );
                }
            }
        };
        check_locale(&langid!("und"));
        // Languages with special casing rules defined in Unicode:
        // https://www.unicode.org/Public/17.0.0/ucd/SpecialCasing.txt
        check_locale(&langid!("tr"));
        check_locale(&langid!("az"));
        check_locale(&langid!("lt"));
    }

    // os_compatible round-trip: new(os_compatible) must produce the same normalized form.
    let pe_rt = PathElement::new(pe.os_compatible(), cs)
        .expect("assertion error: new failed on os_compatible output");
    assert_eq!(
        normalized,
        pe_rt.normalized(),
        "os_compatible round-trip mismatch\n\
         input:         {input:?}\n\
         os_compatible: {:?}\n\
         normalized:    {normalized:?}\n\
         got:           {:?}",
        pe.os_compatible(),
        pe_rt.normalized()
    );
}

fuzz_target!(|data: &[u8]| {
    if data.len() > 255 {
        return;
    }
    fuzz_normalize(data, CaseSensitivity::Sensitive);
    fuzz_normalize(data, CaseSensitivity::Insensitive);
});
