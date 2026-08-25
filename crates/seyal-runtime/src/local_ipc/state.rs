//! Portable Candidate-D connection-state validation.
//!
//! This is production protocol state, not a fuzz-only model. The macOS UDS
//! connection and portable state-machine fuzz target both execute this logic.

use super::framing::MessageType;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    AwaitHello,
    Ready,
    Attached,
    Closing,
}

impl ConnectionState {
    pub fn validate_incoming(self, message_type: MessageType) -> Result<(), StateError> {
        use MessageType::*;
        let allowed = matches!(
            (self, message_type),
            (Self::AwaitHello, ClientHello)
                | (Self::Ready, ListExecutions | Attach | Goodbye)
                | (Self::Attached, Input | Resize | Resync | Detach | Goodbye)
        );
        allowed.then_some(()).ok_or(StateError::InvalidState)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateError {
    InvalidState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_validation_matches_candidate_d_protocol() {
        assert_eq!(
            ConnectionState::AwaitHello.validate_incoming(MessageType::ClientHello),
            Ok(())
        );
        assert_eq!(
            ConnectionState::Ready.validate_incoming(MessageType::Attach),
            Ok(())
        );
        assert_eq!(
            ConnectionState::Attached.validate_incoming(MessageType::Resync),
            Ok(())
        );
        assert_eq!(
            ConnectionState::Ready.validate_incoming(MessageType::Input),
            Err(StateError::InvalidState)
        );
    }
}
