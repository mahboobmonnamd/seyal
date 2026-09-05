mod bounds;
mod mutation;
mod overflow;
mod projection;
mod storage;
mod streaming;
mod transport;

use std::{hint::black_box, mem::size_of, time::Instant};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const CORPUS: &[(&str, &str)] = &[
    ("ascii", "A"),
    ("combining", "e\u{301}"),
    ("cjk-wide", "界"),
    ("emoji-vs16", "❤\u{fe0f}"),
    ("emoji-vs15", "❤\u{fe0e}"),
    ("emoji-modifier", "👍🏽"),
    ("emoji-zwj", "👩‍💻"),
    ("emoji-family", "👨‍👩‍👧‍👦"),
    ("regional-flag", "🇮🇳"),
    ("keycap", "1\u{fe0f}\u{20e3}"),
    ("tamil-combining", "நி"),
    ("arabic-combining", "نِ"),
    ("supplementary-plane", "𐐷"),
    ("isolated-combining", "\u{301}"),
    ("ambiguous-width", "¡"),
];

#[derive(Clone, Debug)]
struct OwnedClusterCell {
    text: String,
    width: u8,
    flags: u8,
}

#[derive(Clone, Copy, Debug)]
struct Inline4Cell {
    scalars: [char; 4],
    len: u8,
    width: u8,
    flags: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ArenaCell {
    offset: u32,
    len: u16,
    width: u8,
    flags: u8,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
struct ScalarBaselineCell {
    scalar: char,
    width: u8,
    flags: u8,
}

fn terminal_width(cluster: &str) -> u8 {
    u8::try_from(UnicodeWidthStr::width(cluster)).unwrap_or(u8::MAX)
}

fn report_corpus() {
    println!("CORPUS\tlabel\tbytes\tscalars\tgraphemes\twidth\twidth_cjk");
    for (label, text) in CORPUS {
        println!(
            "CORPUS\t{label}\t{}\t{}\t{}\t{}\t{}",
            text.len(),
            text.chars().count(),
            text.graphemes(true).count(),
            UnicodeWidthStr::width(*text),
            UnicodeWidthStr::width_cjk(*text),
        );
    }
}

fn report_representation_sizes() {
    println!("SIZE\trepresentation\tbytes");
    println!("SIZE\tscalar-baseline\t{}", size_of::<ScalarBaselineCell>());
    println!("SIZE\towned-string\t{}", size_of::<OwnedClusterCell>());
    println!("SIZE\tinline-4-scalars\t{}", size_of::<Inline4Cell>());
    println!("SIZE\tarena-ref\t{}", size_of::<ArenaCell>());
}

fn benchmark_segmentation(rounds: usize) -> (u128, usize) {
    let mut sample = String::new();
    for (_, text) in CORPUS {
        sample.push_str(text);
        sample.push(' ');
    }

    let started = Instant::now();
    let mut count = 0usize;
    for _ in 0..rounds {
        count = count.wrapping_add(black_box(sample.as_str()).graphemes(true).count());
    }
    (started.elapsed().as_nanos(), black_box(count))
}

fn benchmark_width(rounds: usize) -> (u128, usize) {
    let started = Instant::now();
    let mut sum = 0usize;
    for _ in 0..rounds {
        for (_, cluster) in CORPUS {
            sum = sum.wrapping_add(UnicodeWidthStr::width(black_box(*cluster)));
        }
    }
    (started.elapsed().as_nanos(), black_box(sum))
}

fn benchmark_owned(rounds: usize) -> (u128, usize) {
    let started = Instant::now();
    let mut checksum = 0usize;
    for _ in 0..rounds {
        for (_, cluster) in CORPUS {
            let cell = OwnedClusterCell {
                text: cluster.to_string(),
                width: terminal_width(cluster),
                flags: 0,
            };
            checksum = checksum
                .wrapping_add(cell.text.len())
                .wrapping_add(cell.width as usize)
                .wrapping_add(cell.flags as usize);
            black_box(cell);
        }
    }
    (started.elapsed().as_nanos(), black_box(checksum))
}

fn inline4(cluster: &str) -> (Inline4Cell, bool) {
    let mut scalars = ['\0'; 4];
    let mut len = 0usize;
    let mut overflow = false;
    for scalar in cluster.chars() {
        if len < scalars.len() {
            scalars[len] = scalar;
        } else {
            overflow = true;
        }
        len += 1;
    }
    (
        Inline4Cell {
            scalars,
            len: u8::try_from(len).unwrap_or(u8::MAX),
            width: terminal_width(cluster),
            flags: 0,
        },
        overflow,
    )
}

fn benchmark_inline(rounds: usize) -> (u128, usize, usize) {
    let started = Instant::now();
    let mut checksum = 0usize;
    let mut overflows = 0usize;
    for _ in 0..rounds {
        for (_, cluster) in CORPUS {
            let (cell, overflow) = inline4(cluster);
            overflows += usize::from(overflow);
            checksum = checksum
                .wrapping_add(cell.scalars[0] as usize)
                .wrapping_add(cell.len as usize)
                .wrapping_add(cell.width as usize)
                .wrapping_add(cell.flags as usize);
            black_box(cell);
        }
    }
    (started.elapsed().as_nanos(), black_box(checksum), overflows)
}

fn append_to_arena(arena: &mut Vec<u8>, cluster: &str) -> ArenaCell {
    let offset = u32::try_from(arena.len()).expect("spike arena offset remains bounded");
    let bytes = cluster.as_bytes();
    let len = u16::try_from(bytes.len()).expect("spike cluster remains bounded");
    arena.extend_from_slice(bytes);
    ArenaCell {
        offset,
        len,
        width: terminal_width(cluster),
        flags: 0,
    }
}

fn benchmark_arena(rounds: usize) -> (u128, usize) {
    let corpus_bytes: usize = CORPUS.iter().map(|(_, cluster)| cluster.len()).sum();
    let mut arena = Vec::with_capacity(corpus_bytes);
    let started = Instant::now();
    let mut checksum = 0usize;
    for _ in 0..rounds {
        arena.clear();
        for (_, cluster) in CORPUS {
            let cell = append_to_arena(&mut arena, cluster);
            checksum = checksum
                .wrapping_add(cell.offset as usize)
                .wrapping_add(cell.len as usize)
                .wrapping_add(cell.width as usize)
                .wrapping_add(cell.flags as usize);
            black_box(cell);
        }
        checksum = checksum.wrapping_add(arena.len());
    }
    (started.elapsed().as_nanos(), black_box(checksum))
}

fn ns_per_operation(total_ns: u128, operations: usize) -> f64 {
    total_ns as f64 / operations.max(1) as f64
}

fn main() {
    let representation_rounds = 30_000usize;
    let segmentation_rounds = 20_000usize;
    let width_rounds = 50_000usize;

    println!("SPIKE\tissue\t684");
    println!("SPIKE\tnote\tnon-mergeable comparative evidence only");
    report_representation_sizes();
    report_corpus();
    transport::report_transport_semantics();
    streaming::report_streaming_semantics();
    mutation::report_mutation_semantics();
    storage::report_storage_pressure();
    projection::report_projection_pressure();
    bounds::report_cluster_bounds();
    overflow::report_overflow_policy();

    let (seg_ns, seg_checksum) = benchmark_segmentation(segmentation_rounds);
    println!(
        "BENCH\tsegmentation\trounds={segmentation_rounds}\ttotal_ns={seg_ns}\tchecksum={seg_checksum}"
    );

    let width_ops = width_rounds * CORPUS.len();
    let (width_ns, width_checksum) = benchmark_width(width_rounds);
    println!(
        "BENCH\twidth\tops={width_ops}\ttotal_ns={width_ns}\tns_per_op={:.2}\tchecksum={width_checksum}",
        ns_per_operation(width_ns, width_ops)
    );

    let representation_ops = representation_rounds * CORPUS.len();
    let (owned_ns, owned_checksum) = benchmark_owned(representation_rounds);
    println!(
        "BENCH\towned-string\tops={representation_ops}\ttotal_ns={owned_ns}\tns_per_op={:.2}\tchecksum={owned_checksum}",
        ns_per_operation(owned_ns, representation_ops)
    );

    let (inline_ns, inline_checksum, inline_overflows) = benchmark_inline(representation_rounds);
    println!(
        "BENCH\tinline-4-scalars\tops={representation_ops}\ttotal_ns={inline_ns}\tns_per_op={:.2}\toverflows={inline_overflows}\tchecksum={inline_checksum}",
        ns_per_operation(inline_ns, representation_ops)
    );

    let (arena_ns, arena_checksum) = benchmark_arena(representation_rounds);
    println!(
        "BENCH\tarena-ref\tops={representation_ops}\ttotal_ns={arena_ns}\tns_per_op={:.2}\tchecksum={arena_checksum}",
        ns_per_operation(arena_ns, representation_ops)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_sequences_are_single_extended_graphemes() {
        for (label, text) in CORPUS {
            assert_eq!(
                text.graphemes(true).count(),
                1,
                "corpus item {label} must remain one extended grapheme"
            );
        }
    }

    #[test]
    fn representative_widths_match_terminal_expectations() {
        assert_eq!(UnicodeWidthStr::width("A"), 1);
        assert_eq!(UnicodeWidthStr::width("e\u{301}"), 1);
        assert_eq!(UnicodeWidthStr::width("界"), 2);
        assert_eq!(UnicodeWidthStr::width("👍🏽"), 2);
        assert_eq!(UnicodeWidthStr::width("👩‍💻"), 2);
        assert_eq!(UnicodeWidthStr::width("🇮🇳"), 2);
        assert_eq!(UnicodeWidthStr::width("\u{301}"), 0);
    }

    #[test]
    fn ambiguous_width_policy_is_observable() {
        assert_eq!(UnicodeWidthStr::width("¡"), 1);
        assert_eq!(UnicodeWidthStr::width_cjk("¡"), 2);
    }

    #[test]
    fn arena_reference_round_trips_utf8() {
        let mut arena = Vec::new();
        for (_, cluster) in CORPUS {
            let cell = append_to_arena(&mut arena, cluster);
            let start = cell.offset as usize;
            let end = start + cell.len as usize;
            assert_eq!(std::str::from_utf8(&arena[start..end]).unwrap(), *cluster);
        }
    }

    #[test]
    fn inline_candidate_exposes_real_overflow_case() {
        let (_, overflow) = inline4("👨‍👩‍👧‍👦");
        assert!(overflow, "family ZWJ sequence must exercise overflow path");
    }
}
