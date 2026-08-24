use std::time::Duration;

use crate::{ExecError, platform};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Readiness {
    pub ready: bool,
    pub hangup: bool,
}

impl Readiness {
    #[cfg(target_os = "macos")]
    pub(crate) fn timeout() -> Self {
        Self {
            ready: false,
            hangup: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Interest {
    Read,
    Write,
}

pub(crate) fn wait(
    endpoint: &platform::MasterHandle,
    interest: Interest,
    timeout: Duration,
) -> Result<Readiness, ExecError> {
    platform::wait(endpoint, interest, timeout)
}
