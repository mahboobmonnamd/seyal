use std::{fmt, io};

use seyal_terminal::TerminalError;

#[derive(Debug)]
pub enum ExecError {
    InvalidWindowSize,
    UnsupportedPlatform(&'static str),
    Io(io::Error),
    Terminal(TerminalError),
    ProcessGroupMismatch { expected: i32, actual: i32 },
    IoTimedOut(&'static str),
    TerminationTimedOut,
    StaleRegistrationToken,
}

impl fmt::Display for ExecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWindowSize => f.write_str("PTY rows and columns must both be non-zero"),
            Self::UnsupportedPlatform(message) => f.write_str(message),
            Self::Io(error) => write!(f, "execution I/O error: {error}"),
            Self::Terminal(error) => write!(f, "terminal state error: {error}"),
            Self::ProcessGroupMismatch { expected, actual } => write!(
                f,
                "owned process group changed unexpectedly: expected {expected}, found {actual}"
            ),
            Self::IoTimedOut(operation) => write!(f, "{operation} timed out"),
            Self::TerminationTimedOut => {
                f.write_str("owned child did not reap within the supplied termination policy")
            }
            Self::StaleRegistrationToken => {
                f.write_str("stale execution reactor registration token")
            }
        }
    }
}

impl std::error::Error for ExecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Terminal(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for ExecError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<TerminalError> for ExecError {
    fn from(value: TerminalError) -> Self {
        Self::Terminal(value)
    }
}
