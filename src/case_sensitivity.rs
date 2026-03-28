/// Controls whether path normalization is case-sensitive or case-insensitive.
///
/// This enum is the runtime-dynamic counterpart to the zero-sized marker types
/// [`CaseSensitive`] and [`CaseInsensitive`]. It is used as the type parameter `S`
/// in [`PathElement`](crate::PathElement) when the case sensitivity is not known
/// at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaseSensitivity {
    /// Names differing only in case are treated as distinct.
    Sensitive,
    /// Names differing only in case produce the same normalized name.
    Insensitive,
}

/// Zero-sized type-level marker for case-sensitive normalization.
///
/// This is the type parameter `S` in [`PathElementCS`](crate::PathElementCS).
/// Converts to [`CaseSensitivity::Sensitive`] via the [`From`] impl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaseSensitive;

/// Zero-sized type-level marker for case-insensitive normalization.
///
/// This is the type parameter `S` in [`PathElementCI`](crate::PathElementCI).
/// Converts to [`CaseSensitivity::Insensitive`] via the [`From`] impl.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaseInsensitive;

impl From<CaseSensitive> for CaseSensitivity {
    fn from(_: CaseSensitive) -> Self {
        CaseSensitivity::Sensitive
    }
}

impl From<CaseInsensitive> for CaseSensitivity {
    fn from(_: CaseInsensitive) -> Self {
        CaseSensitivity::Insensitive
    }
}

impl From<&CaseSensitive> for CaseSensitivity {
    fn from(_: &CaseSensitive) -> Self {
        CaseSensitivity::Sensitive
    }
}

impl From<&CaseInsensitive> for CaseSensitivity {
    fn from(_: &CaseInsensitive) -> Self {
        CaseSensitivity::Insensitive
    }
}

impl From<&CaseSensitivity> for CaseSensitivity {
    fn from(s: &CaseSensitivity) -> Self {
        *s
    }
}

#[cfg(test)]
mod tests {
    #[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::{CaseInsensitive, CaseSensitive, CaseSensitivity};

    #[test]
    fn from_case_sensitive() {
        assert_eq!(
            CaseSensitivity::from(CaseSensitive),
            CaseSensitivity::Sensitive
        );
    }

    #[test]
    fn from_case_insensitive() {
        assert_eq!(
            CaseSensitivity::from(CaseInsensitive),
            CaseSensitivity::Insensitive
        );
    }
}
