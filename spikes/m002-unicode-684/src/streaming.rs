use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Policy {
    LegacyScalar,
    GraphemeMutableHypothesis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LateWidenEdgePolicy {
    DetectConflict,
    Mode2027AutowrapReflowHypothesis,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StreamStats {
    cursor_col: usize,
    wraps: usize,
    joined_scalars: usize,
    late_widens: usize,
    late_narrows: usize,
    late_widen_reflows: usize,
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
    edge_policy: LateWidenEdgePolicy,
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
            match edge_policy {
                LateWidenEdgePolicy::DetectConflict => {
                    stats.right_edge_widen_conflicts += 1;
                }
                LateWidenEdgePolicy::Mode2027AutowrapReflowHypothesis => {
                    // The mode-2027 contract permits VS16 to widen the active
                    // cluster and move that same cluster to the next row when
                    // DECAWM is enabled. This remains spike evidence until the
                    // history/reflow model in #685 proves the physical/logical
                    // line transition.
                    stats.wraps += 1;
                    stats.late_widen_reflows += 1;
                    stats.cursor_col = active.committed_width;
                }
            }
        }
    } else if natural_width < active.committed_width {
        // A narrowing selector that is still part of the active, appendable
        // grapheme is safe to apply: no following cell can have been committed
        // between two no-break scalars. The terminal can release the former
        // continuation and move the cursor back by the width delta. Once the
        // active anchor is invalidated, mutation.rs prevents this path.
        let delta = active.committed_width - natural_width;
        active.committed_width = natural_width;
        stats.late_narrows += 1;
        stats.cursor_col = stats.cursor_col.saturating_sub(delta);
    }
}

fn simulate_grapheme_mutable(
    text: &str,
    columns: usize,
    start_col: usize,
    edge_policy: LateWidenEdgePolicy,
) -> StreamStats {
    let mut stats = StreamStats {
        cursor_col: start_col,
        ..StreamStats::default()
    };
    let mut active = ActiveCluster::default();

    for scalar in text.chars() {
        if active.text.is_empty() {
            start_cluster(&mut active, scalar, &mut stats, columns);
        } else if starts_same_grapheme(active.text.as_str(), scalar) {
            append_to_cluster(&mut active, scalar, &mut stats, columns, edge_policy);
        } else {
            start_cluster(&mut active, scalar, &mut stats, columns);
        }
    }
    stats
}

fn simulate(policy: Policy, text: &str, columns: usize, start_col: usize) -> StreamStats {
    match policy {
        Policy::LegacyScalar => simulate_legacy(text, columns, start_col),
        Policy::GraphemeMutableHypothesis => simulate_grapheme_mutable(
            text,
            columns,
            start_col,
            LateWidenEdgePolicy::DetectConflict,
        ),
    }
}

pub(crate) fn report_streaming_semantics() {
    const SEQUENCES: &[(&str, &str)] = &[
        ("combining", "e\u{301}"),
        ("late-wide-vs16", "❤\u{fe0f}"),
        ("late-narrow-vs15", "😐\u{fe0e}"),
        ("emoji-zwj", "👩‍💻"),
        ("emoji-family", "👨‍👩‍👧‍👦"),
        ("regional-flag", "🇮🇳"),
        ("tamil-combining", "நி"),
    ];

    println!(
        "STREAM\tlabel\tpolicy\tcursor\twraps\tjoined\tlate_widens\tlate_narrows\tlate_widen_reflows\tright_edge_conflicts"
    );
    for (label, text) in SEQUENCES {
        for policy in [Policy::LegacyScalar, Policy::GraphemeMutableHypothesis] {
            let stats = simulate(policy, text, 80, 0);
            println!(
                "STREAM\t{label}\t{policy:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                stats.cursor_col,
                stats.wraps,
                stats.joined_scalars,
                stats.late_widens,
                stats.late_narrows,
                stats.late_widen_reflows,
                stats.right_edge_widen_conflicts,
            );
        }
    }

    let conflict =
        simulate_grapheme_mutable("❤\u{fe0f}", 80, 79, LateWidenEdgePolicy::DetectConflict);
    let reflow = simulate_grapheme_mutable(
        "❤\u{fe0f}",
        80,
        79,
        LateWidenEdgePolicy::Mode2027AutowrapReflowHypothesis,
    );
    println!(
        "STREAM_EDGE\tlate-wide-at-right-edge\tconflict_cursor={}\tconflicts={}\treflow_cursor={}\treflows={}\twraps={}",
        conflict.cursor_col,
        conflict.right_edge_widen_conflicts,
        reflow.cursor_col,
        reflow.late_widen_reflows,
        reflow.wraps,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combining_mark_joins_without_extra_cursor_width() {
        let stats = simulate(Policy::GraphemeMutableHypothesis, "e\u{301}", 80, 0);
        assert_eq!(stats.cursor_col, 1);
        assert_eq!(stats.joined_scalars, 1);
        assert_eq!(stats.late_widens, 0);
    }

    #[test]
    fn variation_selector_can_widen_an_existing_active_cluster() {
        let stats = simulate(Policy::GraphemeMutableHypothesis, "❤\u{fe0f}", 80, 0);
        assert_eq!(stats.cursor_col, 2);
        assert_eq!(stats.joined_scalars, 1);
        assert_eq!(stats.late_widens, 1);
    }

    #[test]
    fn variation_selector_can_narrow_only_while_cluster_is_active() {
        let stats = simulate(Policy::GraphemeMutableHypothesis, "😐\u{fe0e}", 80, 0);
        assert_eq!(stats.cursor_col, 1);
        assert_eq!(stats.joined_scalars, 1);
        assert_eq!(stats.late_narrows, 1);
    }

    #[test]
    fn late_widen_at_right_edge_is_explicitly_detected() {
        let stats = simulate(Policy::GraphemeMutableHypothesis, "❤\u{fe0f}", 80, 79);
        assert_eq!(stats.right_edge_widen_conflicts, 1);
    }

    #[test]
    fn mode_2027_autowrap_candidate_reflows_late_widened_cluster() {
        let stats = simulate_grapheme_mutable(
            "❤\u{fe0f}",
            80,
            79,
            LateWidenEdgePolicy::Mode2027AutowrapReflowHypothesis,
        );
        assert_eq!(stats.right_edge_widen_conflicts, 0);
        assert_eq!(stats.late_widen_reflows, 1);
        assert_eq!(stats.wraps, 1);
        assert_eq!(stats.cursor_col, 2);
    }

    #[test]
    fn family_emoji_is_one_terminal_cluster_under_grapheme_hypothesis() {
        let stats = simulate(Policy::GraphemeMutableHypothesis, "👨‍👩‍👧‍👦", 80, 0);
        assert_eq!(stats.cursor_col, 2);
        assert_eq!(stats.joined_scalars, 6);
    }

    #[test]
    fn legacy_scalar_and_grapheme_models_are_observably_different() {
        let legacy = simulate(Policy::LegacyScalar, "👩‍💻", 80, 0);
        let grapheme = simulate(Policy::GraphemeMutableHypothesis, "👩‍💻", 80, 0);
        assert_ne!(legacy.cursor_col, grapheme.cursor_col);
        assert_eq!(grapheme.cursor_col, 2);
    }
}
