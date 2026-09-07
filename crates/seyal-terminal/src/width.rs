//! Terminal occupation width policy for canonical graphemes (SPEC-011 §4/§7).

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

/// East Asian Ambiguous characters occupy one cell by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AmbiguousWidthPolicy {
    #[default]
    Narrow,
    Wide,
}

/// Returns whether `next` continues the same extended grapheme as `existing`.
pub(crate) fn extends_active_grapheme(existing: &str, next: char) -> bool {
    if existing.is_empty() {
        return false;
    }
    let mut combined = String::with_capacity(existing.len() + next.len_utf8());
    combined.push_str(existing);
    combined.push(next);
    combined.graphemes(true).count() == 1
}

/// Terminal occupation width for a complete canonical grapheme payload.
///
/// Width domain is 0, 1, or 2. Zero-width combining-only units are rare after a
/// base scalar; isolated combining marks yield width 0.
pub(crate) fn grapheme_terminal_width(text: &str, ambiguous: AmbiguousWidthPolicy) -> u8 {
    if text.is_empty() {
        return 0;
    }

    // Variation selectors / ZWJ / skin-tone may widen emoji presentation.
    if text.chars().any(|c| c == '\u{FE0F}') {
        // Explicit emoji presentation: at least width 2 when a base is present.
        if text.chars().any(|c| !is_nonspacing_mark_or_format(c)) {
            return 2;
        }
    }

    let mut width = 0u32;
    for ch in text.chars() {
        if is_nonspacing_mark_or_format(ch) {
            continue;
        }
        let w = match ambiguous {
            AmbiguousWidthPolicy::Narrow => ch.width().unwrap_or(1),
            AmbiguousWidthPolicy::Wide => ch.width_cjk().unwrap_or(1),
        };
        width = width.saturating_add(w as u32);
    }

    match width {
        0 => 0,
        1 => 1,
        _ => 2,
    }
}

fn is_nonspacing_mark_or_format(ch: char) -> bool {
    matches!(
        ch,
        '\u{FE0E}' | '\u{FE0F}' | '\u{200D}' | '\u{20E3}' | '\u{FE00}'..='\u{FE0D}'
    ) || UnicodeWidthChar::width(ch) == Some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_width_one() {
        assert_eq!(
            grapheme_terminal_width("A", AmbiguousWidthPolicy::Narrow),
            1
        );
    }

    #[test]
    fn cjk_width_two() {
        assert_eq!(
            grapheme_terminal_width("世", AmbiguousWidthPolicy::Narrow),
            2
        );
    }

    #[test]
    fn heart_alone_width_one() {
        assert_eq!(
            grapheme_terminal_width("❤", AmbiguousWidthPolicy::Narrow),
            1
        );
    }

    #[test]
    fn heart_vs16_width_two() {
        assert_eq!(
            grapheme_terminal_width("❤️", AmbiguousWidthPolicy::Narrow),
            2
        );
    }

    #[test]
    fn extends_combining() {
        assert!(extends_active_grapheme("e", '\u{0301}'));
        assert!(!extends_active_grapheme("e", 'x'));
    }
}
