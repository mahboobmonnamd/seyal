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

    // Cargo unifies features across a workspace, so `required-features` does
    // not distinguish ordinary `cargo bench --workspace` from an intentional
    // controlled-host calibration. Require a dedicated argument as well as
    // operator authorization: this prevents the workspace bench and the
    // explicit task command from both running the expensive 2,400-cycle suite.
    if args
        .get(1)
        .is_none_or(|arg| arg != "--controlled-calibration")
    {
        println!(
            "pass9_preimplementation_calibration SKIPPED reason=explicit_controlled_calibration_argument_required required_arg=--controlled-calibration {PERFORMANCE_CLAIM}"
        );
        return;
    }
    if std::env::var("SEYAL_RUN_PASS9_CALIBRATION").as_deref() != Ok("1") {
        println!(
            "pass9_preimplementation_calibration SKIPPED reason=operator_opt_in_required opt_in_env=SEYAL_RUN_PASS9_CALIBRATION=1 {PERFORMANCE_CLAIM}"
        );
        return;
    }
    orchestrator::run_orchestrator();
}
