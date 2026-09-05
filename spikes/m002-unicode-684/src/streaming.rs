use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Policy {
    LegacyScalar,
    GraphemeMonotonicHypothesis,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StreamStats {
    cursor_col: usize,
    wraps: usize,
    joined_scalars: usize,
    late_widens: usize,
    suppressed_narrows: usize,
    right_edge_widen_conflicts: usize,
}

#[derive(Debug, Default)]
struct ActiveCluster {
    text: String,
    committed_width: usize,
}

fn place_new_width(stats: &mut StreamStats, width: usize, columns: usize) {
    if width == 0 {
        return;
    }
    if stats.cursor_col + width > columns {
        stats.wraps += 1;
        stats.cursor_col = 0;
    }
    stats.cursor_col += width;
}

fn simulate_legacy(text: &str, columns: usize, start_col: usize) -> StreamStats {
    let mut stats = StreamStats {
        cursor_col: start_col,
        ..StreamStats::default()
    };
    for scalar in text.chars() {
        place_new_width(&mut stats, scalar.width().unwrap_or(0), columns);
    }
    stats
}

fn starts_same_grapheme(active: &str, scalar: char) -> bool {
    let mut candidate = String::with_capacity(active.len() + scalar.len_utf8());
    candidate.push_str(active);
    candidate.push(scalar);
    candidate.graphemes(true).count() == 1
}

fn start_cluster(
    active: &mut ActiveCluster,
    scalar: char,
    stats: &mut StreamStats,
    columns: usize,
) {
    active.text.clear();
    active.text.push(scalar);
    active.committed_width = UnicodeWidthStr::width(active.text.as_str());
    place_new_width(stats, active.committed_width, columns);
}

fn append_to_cluster(
    active: &mut ActiveCluster,
    scalar: char,
    stats: &mut StreamStats,
    columns: usize,
) {
    active.text.push(scalar);
    stats.joined_scalars += 1;

    let natural_width = UnicodeWidthStr::width(active.text.as_str());
    if natural_width > active.committed_width {
        let delta = natural_width - active.committed_width;
        active.committed_width = natural_width;
        stats.late_widens += 1;
        stats.cursor_col += delta;
        if stats.cursor_col > columns {
            stats.right_edge_widen_conflicts += 1;
        }
    } else if natural_width < active.committed_width {
        // This is intentionally a hypothesis to measure, not an accepted Seyal rule.
        // A terminal cannot safely move already-emitted following cells backwards
        // without an explicit compatibility contract, so record the event instead.
        stats.suppressed_narrows += 1;
    }
}

fn simulate_grapheme_monotonic(text: &str, columns: usize, start_col: usize) -> StreamStats {
    let mut stats = StreamStats {
        cursor_col: start_col,
        ..StreamStats::default()
    };
    let mut active = ActiveCluster::default();

    for scalar in text.chars() {
        if active.text.is_empty() {
            start_cluster(&mut active, scalar, &mut stats, columns);
        } else if starts_same_grapheme(active.text.as_str(), scalar) {
            append_to_cluster(&mut active, scalar, &mut stats, columns);
        } else {
            start_cluster(&mut active, scalar, &mut stats, columns);
        }
    }
    stats
}

fn simulate(policy: Policy, text: &str, columns: usize, start_col: usize) -> StreamStats {
    match policy {
        Policy::LegacyScalar => simulate_legacy(text, columns, start_col),
        Policy::GraphemeMonotonicHypothesis => {
            simulate_grapheme_monotonic(text, columns, start_col)
        }
    }
}

pub(crate) fn report_streaming_semantics() {
    const SEQUENCES: &[(&str, &str)] = &[
        ("combining", "e\u{301}"),
        ("late-wide-vs16", "❤\u{fe0f}"),
        ("emoji-zwj", "👩‍💻"),
        ("emoji-family", "👨‍👩‍👧‍👦"),
        ("regional-flag", "🇮🇳"),
        ("tamil-combining", "நி"),
    ];

    println!(
        "STREAM\tlabel\tpolicy\tcursor\twraps\tjoined\tlate_widens\tsuppressed_narrows\tright_edge_conflicts"
    );
    for (label, text) in SEQUENCES {
        for policy in [Policy::LegacyScalar, Policy::GraphemeMonotonicHypothesis] {
            let stats = simulate(policy, text, 80, 0);
            println!(
                "STREAM\t{label}\t{policy:?}\t{}\t{}\t{}\t{}\t{}\t{}",
                stats.cursor_col,
                stats.wraps,
                stats.joined_scalars,
                stats.late_widens,
                stats.suppressed_narrows,
                stats.right_edge_widen_conflicts,
            );
        }
    }

    // Heart starts as width 1 at the final column; VS16 then makes the same
    // extended grapheme width 2. The spike records this as an unresolved edge
    // event rather than inventing wrap semantics here.
    let edge = simulate(Policy::GraphemeMonotonicHypothesis, "❤\u{fe0f}", 80, 79);
    println!(
        "STREAM_EDGE\tlate-wide-at-right-edge\tcursor={}\tconflicts={}",
        edge.cursor_col, edge.right_edge_widen_conflicts
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combining_mark_joins_without_extra_cursor_width() {
        let stats = simulate(Policy::GraphemeMonotonicHypothesis, "e\u{301}", 80, 0);
        assert_eq!(stats.cursor_col, 1);
        assert_eq!(stats.joined_scalars, 1);
        assert_eq!(stats.late_widens, 0);
    }

    #[test]
    fn variation_selector_can_widen_an_existing_cluster() {
        let stats = simulate(Policy::GraphemeMonotonicHypothesis, "❤\u{fe0f}", 80, 0);
        assert_eq!(stats.cursor_col, 2);
        assert_eq!(stats.joined_scalars, 1);
        assert_eq!(stats.late_widens, 1);
    }

    #[test]
    fn late_widen_at_right_edge_is_explicitly_detected() {
        let stats = simulate(Policy::GraphemeMonotonicHypothesis, "❤\u{fe0f}", 80, 79);
        assert_eq!(stats.right_edge_widen_conflicts, 1);
    }

    #[test]
    fn family_emoji_is_one_terminal_cluster_under_grapheme_hypothesis() {
        let stats = simulate(Policy::GraphemeMonotonicHypothesis, "👨‍👩‍👧‍👦", 80, 0);
        assert_eq!(stats.cursor_col, 2);
        assert_eq!(stats.joined_scalars, 6);
    }

    #[test]
    fn legacy_scalar_and_grapheme_models_are_observably_different() {
        let legacy = simulate(Policy::LegacyScalar, "👩‍💻", 80, 0);
        let grapheme = simulate(Policy::GraphemeMonotonicHypothesis, "👩‍💻", 80, 0);
        assert_ne!(legacy.cursor_col, grapheme.cursor_col);
        assert_eq!(grapheme.cursor_col, 2);
    }
}
