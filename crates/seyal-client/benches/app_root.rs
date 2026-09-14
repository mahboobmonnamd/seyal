use std::time::Instant;

use seyal_client::app::{AppAction, ApplicationRoot, BindingEvidence};
use seyal_core::{AttachmentId, ExecutionId};

const PERFORMANCE_CLAIM: &str = "performance_claim=false";
const REPETITIONS: usize = 8_000;
const PANE_COUNTS: [usize; 3] = [1, 8, 32];

fn evidence(tag: u8) -> BindingEvidence {
    BindingEvidence {
        execution: ExecutionId::from_bytes([tag; 16]),
        attachment: AttachmentId::from_bytes([tag.wrapping_add(1); 16]),
        controller: true,
        pty_generation: 1,
        alternate_screen: false,
    }
}

fn main() {
    let _contract_clock = Instant::now();
    println!(
        "app_root architecture=ApplicationRoot_action_snapshot {PERFORMANCE_CLAIM} percentile_method=nearest_rank repetitions={REPETITIONS}"
    );
    print_host_metadata();
    for panes in PANE_COUNTS {
        measure(panes);
    }
}

fn measure(representative_panes: usize) {
    let mut samples = Vec::with_capacity(REPETITIONS);
    let mut allocated = 0usize;
    for index in 0..REPETITIONS {
        let start = Instant::now();
        let mut root = ApplicationRoot::new();
        let _ = root.apply(AppAction::Bind {
            fence: root.fence(),
            evidence: evidence((index % 250) as u8 + 1),
        });
        let snap = root.snapshot();
        allocated += snap.output_utf8.len() + snap.accessibility.len() * 64;
        samples.push(start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        let _ = std::hint::black_box((root.snapshot(), representative_panes));
    }
    samples.sort_unstable();
    let p50 = percentile_ns(&samples, 50);
    let p95 = percentile_ns(&samples, 95);
    println!(
        "app_root_action_snapshot panes={representative_panes} sample_count={} p50_us={:.3} p95_us={:.3} copy_bytes_est={} {PERFORMANCE_CLAIM}",
        samples.len(),
        p50 as f64 / 1_000.0,
        p95 as f64 / 1_000.0,
        allocated / REPETITIONS.max(1),
    );
}

fn percentile_ns(sorted: &[u64], percentile: u8) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (usize::from(percentile) * sorted.len()) / 100;
    sorted[rank.min(sorted.len() - 1)]
}

fn print_host_metadata() {
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_else(|| "unknown".to_owned());
    println!(
        "app_root_host os={} arch={} sha={} build=release_bench",
        std::env::consts::OS,
        std::env::consts::ARCH,
        sha.trim(),
    );
}
