use std::time::{Duration, Instant};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{Runtime, RuntimeConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = Runtime::new(RuntimeConfig::m001()?)?;
    let mut args = std::env::args_os().skip(1);
    let program = args
        .next()
        .or_else(|| std::env::var_os("SHELL"))
        .unwrap_or_else(|| "/bin/sh".into());
    let command = CommandSpec::new(program).args(args);
    runtime.create_execution(command, WindowSize::new(80, 24, 0, 0)?)?;

    while runtime.execution_count() != 0 {
        runtime.poll_once(Some(Duration::from_secs(30)))?;
    }

    runtime.begin_shutdown()?;
    runtime.run_until_empty(Instant::now() + Duration::from_secs(3))?;
    Ok(())
}
