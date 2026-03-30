#![no_main]

use icu_casemap::CaseMapper;
use icu_locale_core::langid;
use libfuzzer_sys::fuzz_target;
use normalized_path::test_helpers::{
    apple_compatible_from_normalized_cs, case_fold, decode_utf8_lossy,
    encode_java_modified_utf8, is_reserved_on_windows, map_control_chars, map_fullwidth,
    map_turkish_i, nfc, nfd, normalize_ci_from_normalized_cs, normalize_cs,
    trim_whitespace_like, validate_path_element, windows_compatible_from_normalized_cs,
};
#[cfg(target_vendor = "apple")]
use normalized_path::test_helpers::apple_compatible_from_normalized_cs_fallback;
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
    // Construct via from_bytes — also exercises the UTF-8 rejection path.
    let Ok(pe) = PathElement::from_bytes(data, cs) else {
        return;
    };
    let input = pe.original();
    let normalized = pe.normalized();
    validate_path_element(normalized).expect("validate_path_element must accept normalized output");

    // decode_utf8_lossy of the raw data must produce the same original.
    let decoded = decode_utf8_lossy(data);
    assert_eq!(
        input, &*decoded,
        "from_bytes original does not match decode_utf8_lossy\n\
         data:    {data:?}\n\
         original: {input:?}\n\
         decoded:  {decoded:?}"
    );

    // Constructing from the decoded string must produce the same normalized form.
    let pe_decoded = PathElement::new(&*decoded, cs)
        .expect("assertion error: PathElement::new failed on decode_utf8_lossy output");
    assert_eq!(
        normalized,
        pe_decoded.normalized(),
        "normalize mismatch between from_bytes and new(decode_utf8_lossy(data))\n\
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

    // Converting the original to Java Modified UTF-8 and back via from_bytes
    // must produce the same normalized form.
    let mutf8 = encode_java_modified_utf8(input);
    let pe_mutf8 = PathElement::from_bytes(&*mutf8, cs)
        .expect("assertion error: PathElement::from_bytes failed on MUTF-8 encoded input");
    assert_eq!(
        normalized,
        pe_mutf8.normalized(),
        "normalize mismatch after encode_java_modified_utf8 roundtrip\n\
         input:   {input:?}\n\
         mutf8:   {mutf8:?}\n\
         normalized:       {normalized:?}\n\
         got:              {:?}",
        pe_mutf8.normalized()
    );

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
        pe.original().as_bytes() == pe.os_compatible(),
        "is_os_compatible mismatch\n\
         original:      {:?}\n\
         os_compatible: {:?}",
        pe.original(),
        String::from_utf8_lossy(pe.os_compatible())
    );

    // is_reserved_on_windows must be stable under NFD and NFD→casefold→NFD.
    assert_eq!(
        is_reserved_on_windows(normalized),
        is_reserved_on_windows(&nfd(normalized)),
        "is_reserved_on_windows mismatch after nfd\n\
         normalized: {normalized:?}"
    );
    assert_eq!(
        is_reserved_on_windows(normalized),
        is_reserved_on_windows(&nfd(&case_fold(&nfd(normalized)))),
        "is_reserved_on_windows mismatch after nfd(case_fold(nfd(...)))\n\
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
        core::str::from_utf8(&windows_compatible_from_normalized_cs(input)).unwrap(),
    );
    check(
        "apple_compatible_from_normalized_cs",
        core::str::from_utf8(&apple_compatible_from_normalized_cs(input).unwrap()).unwrap(),
    );
    #[cfg(target_vendor = "apple")]
    check(
        "apple_compatible_from_normalized_cs_fallback",
        core::str::from_utf8(&apple_compatible_from_normalized_cs_fallback(input)).unwrap(),
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
        // Turkish İ mapping is only applied in CI mode.
        check("map_turkish_i", &map_turkish_i(input));

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
        check(
            "icu_lowercase",
            &cm.lowercase_to_string(case_input, &langid!("und")),
        );
        check(
            "icu_uppercase",
            &cm.uppercase_to_string(case_input, &langid!("und")),
        );
        check(
            "icu_titlecase",
            &cm.titlecase_segment_with_only_case_data_to_string(
                case_input,
                &langid!("und"),
                Default::default(),
            ),
        );
        check(
            "icu_lowercase_tr",
            &cm.lowercase_to_string(case_input, &langid!("tr")),
        );
        check(
            "icu_uppercase_tr",
            &cm.uppercase_to_string(case_input, &langid!("tr")),
        );
        check(
            "icu_titlecase_tr",
            &cm.titlecase_segment_with_only_case_data_to_string(
                case_input,
                &langid!("tr"),
                Default::default(),
            ),
        );
        check("fold_turkic", &cm.fold_turkic_string(case_input));
    }

    // os_compatible round-trip: from_bytes(os_compatible) must produce the same normalized form.
    let pe_rt = PathElement::from_bytes(pe.os_compatible(), cs)
        .expect("assertion error: from_bytes failed on os_compatible output");
    assert_eq!(
        normalized,
        pe_rt.normalized(),
        "os_compatible round-trip mismatch\n\
         input:         {input:?}\n\
         os_compatible: {:?}\n\
         normalized:    {normalized:?}\n\
         got:           {:?}",
        String::from_utf8_lossy(pe.os_compatible()),
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
