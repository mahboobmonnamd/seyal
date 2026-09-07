//! Pinned Unicode semantic-data version for M002 (#815 / SPEC-011).

/// Unicode Standard version that owns grapheme-breaking and East Asian Width
/// decisions in this crate. Must stay synchronized with the selected
/// `unicode-segmentation` / width tables and with SPEC-011.
pub const UNICODE_SEMANTIC_VERSION: &str = "17.0.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_unicode_version_is_exact() {
        assert_eq!(UNICODE_SEMANTIC_VERSION, "17.0.0");
    }
}
