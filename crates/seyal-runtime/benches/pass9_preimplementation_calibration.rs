use std::time::Instant;

// A `[[bench]]` root file resolves child `mod` declarations against its own
// directory, not a directory named after the file — unlike an ordinary
// submodule. `#[path]` places these under a dedicated subdirectory instead
// of mixing them into `benches/` alongside the other Pass 5/7/8 harnesses.
#[cfg(target_os = "macos")]
#[path = "pass9_preimplementation_calibration/cohort.rs"]
mod cohort;
#[cfg(target_os = "macos")]
#[path = "pass9_preimplementation_calibration/config.rs"]
mod config;
#[cfg(target_os = "macos")]
#[path = "pass9_preimplementation_calibration/metrics.rs"]
mod metrics;
#[cfg(target_os = "macos")]
#[path = "pass9_preimplementation_calibration/orchestrator.rs"]
mod orchestrator;
#[cfg(target_os = "macos")]
#[path = "pass9_preimplementation_calibration/process_io.rs"]
mod process_io;
#[cfg(target_os = "macos")]
#[path = "pass9_preimplementation_calibration/protocol.rs"]
mod protocol;
#[cfg(target_os = "macos")]
#[path = "pass9_preimplementation_calibration/worker.rs"]
mod worker;

#[cfg(target_os = "macos")]
use config::{Geometry, LossMode};

const PERFORMANCE_CLAIM: &str = "performance_claim=false";

fn main() {
    let _contract_clock = Instant::now();

    #[cfg(not(target_os = "macos"))]
    println!(
        "pass9_preimplementation_calibration PLATFORM_LIMITED target_os!=macos {PERFORMANCE_CLAIM}"
    );

    #[cfg(target_os = "macos")]
    run_macos();
}

#[cfg(target_os = "macos")]
fn run_macos() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg| arg == "--runtime-worker") {
        let geometry = Geometry::parse(args.get(2).expect("worker geometry"));
        worker::run_runtime_worker(geometry);
        return;
    }
    if args.get(1).is_some_and(|arg| arg == "--cohort") {
        let mode = LossMode::parse(args.get(2).expect("cohort mode"));
        let geometry = Geometry::parse(args.get(3).expect("cohort geometry"));
        let cohort: usize = args
            .get(4)
            .expect("cohort number")
            .parse()
            .expect("cohort integer");
        cohort::run_cohort(mode, geometry, cohort);
        return;
    }
    orchestrator::run_orchestrator();
}
