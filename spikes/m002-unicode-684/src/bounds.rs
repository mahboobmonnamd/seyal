use std::{hint::black_box, time::Instant};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

fn combining_storm(mark_count: usize) -> String {
    let mut text = String::with_capacity(1 + mark_count * 2);
    text.push('a');
    for _ in 0..mark_count {
        text.push('\u{301}');
    }
    text
}

fn naive_incremental_resegment(text: &str) -> usize {
    let mut active = String::new();
    let mut checksum = 0usize;
    for scalar in text.chars() {
        active.push(scalar);
        checksum = checksum.wrapping_add(black_box(active.as_str()).graphemes(true).count());
        checksum = checksum.wrapping_add(UnicodeWidthStr::width(black_box(active.as_str())));
    }
    black_box(checksum)
}

pub(crate) fn report_cluster_bounds() {
    println!("BOUND\tmarks\tbytes\tscalars\tgraphemes\twidth\tnaive_total_ns");
    for marks in [4usize, 64, 256, 1_024, 4_096] {
        let text = combining_storm(marks);
        let started = Instant::now();
        let checksum = naive_incremental_resegment(&text);
        let elapsed = started.elapsed().as_nanos();
        println!(
            "BOUND\t{marks}\t{}\t{}\t{}\t{}\t{elapsed}\tchecksum={checksum}",
            text.len(),
            text.chars().count(),
            text.graphemes(true).count(),
            UnicodeWidthStr::width(text.as_str()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_does_not_supply_a_small_fixed_cluster_bound() {
        let text = combining_storm(4_096);
        assert_eq!(text.graphemes(true).count(), 1);
        assert_eq!(UnicodeWidthStr::width(text.as_str()), 1);
        assert_eq!(text.chars().count(), 4_097);
        assert!(text.len() > 8_000);
    }

    #[test]
    fn naive_incremental_probe_revisits_growing_cluster_state() {
        let small = combining_storm(8);
        let large = combining_storm(64);
        let small_checksum = naive_incremental_resegment(&small);
        let large_checksum = naive_incremental_resegment(&large);
        assert!(large_checksum > small_checksum);
    }
}
