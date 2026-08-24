use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use seyal_exec::{CommandSpec, WindowSize};
#[cfg(target_os = "macos")]
use seyal_runtime::{Runtime, RuntimeConfig};

fn main() {
    #[cfg(not(target_os = "macos"))]
    {
        println!(
            "seyal-runtime scalability: PLATFORM_LIMITED target_os!=macos; no performance claim"
        );
    }

    #[cfg(target_os = "macos")]
    run_macos();
}

#[cfg(target_os = "macos")]
fn run_macos() {
    let populations = [0usize, 1, 10, 50, 100];
    println!("seyal-runtime Pass 4 benchmark; performance_claim=false");
    println!("profile=80x24 primary-active alternate-inactive minimal-scrollback");

    for population in populations {
        let mut config = RuntimeConfig::m001().expect("bundled capability policy");
        config.singleton_path = std::env::temp_dir().join(format!(
            "seyal-runtime-bench-{}-{population}.lock",
            std::process::id()
        ));
        config.max_executions = population.max(1);
        let mut runtime = Runtime::new(config).expect("headless runtime");
        let before = Instant::now();
        let mut created = 0usize;
        for _ in 0..population {
            let command = CommandSpec::new("/bin/sh").args(["-c", "sleep 30"]);
            match runtime
                .create_execution(command, WindowSize::new(80, 24, 0, 0).expect("valid size"))
            {
                Ok(_) => created += 1,
                Err(error) => {
                    println!(
                        "population={population} created={created} classification=PLATFORM_LIMITED error={error}"
                    );
                    break;
                }
            }
        }
        let creation = before.elapsed();
        println!(
            "population={population} created={created} create_us={} aggregate_pending={} threads_model=single-reactor-owner",
            creation.as_micros(),
            runtime.aggregate_accepted_but_unwritten_bytes()
        );

        let teardown = Instant::now();
        runtime.begin_shutdown().expect("begin shutdown");
        let result = runtime.run_until_empty(Instant::now() + Duration::from_secs(5));
        println!(
            "population={population} teardown_us={} shutdown={:?}",
            teardown.elapsed().as_micros(),
            result.as_ref().map(|_| "ok")
        );
        result.expect("controlled Runtime teardown");
    }
}
