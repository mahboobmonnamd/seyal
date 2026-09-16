//! SPEC-006 §21.5 TerminalKeyV2 Error correlation.
//!
//! Runtime echoes a nonzero `detail_code=action_id` for structurally readable
//! V2 rejections. The client validates that ID against connection-local
//! sent/highest-error bounds and keeps the connection for ordinary
//! authorization, unsupported encoding, and queue-capacity rejections.

use seyal_protocol::framing::ErrorCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum V2IncomingDisposition {
    ClientBackpressure,
    LostController,
    Protocol,
    Fatal(ErrorCode),
}

/// Classify a type-29 Error. The returned `u32` is the updated highest-error
/// bound; it changes only when a correlated nonzero ID is accepted.
pub(crate) fn classify_v2_incoming_error(
    error_code: u16,
    detail_code: u32,
    last_sent: u32,
    highest_error: u32,
) -> (V2IncomingDisposition, u32) {
    if detail_code == 0 {
        let disposition = match ErrorCode::from_u16(error_code) {
            Some(ErrorCode::Backpressure)
            | Some(ErrorCode::PermissionDenied)
            | Some(ErrorCode::StaleIdentity)
            | Some(ErrorCode::InvalidExecution) => V2IncomingDisposition::Protocol,
            Some(code) => V2IncomingDisposition::Fatal(code),
            None => V2IncomingDisposition::Protocol,
        };
        return (disposition, highest_error);
    }
    if detail_code > last_sent || detail_code <= highest_error {
        return (V2IncomingDisposition::Protocol, highest_error);
    }
    let disposition = match ErrorCode::from_u16(error_code) {
        Some(ErrorCode::Backpressure) | Some(ErrorCode::MalformedPayload) => {
            V2IncomingDisposition::ClientBackpressure
        }
        Some(ErrorCode::PermissionDenied)
        | Some(ErrorCode::StaleIdentity)
        | Some(ErrorCode::InvalidExecution) => V2IncomingDisposition::LostController,
        Some(code) => V2IncomingDisposition::Fatal(code),
        None => V2IncomingDisposition::Protocol,
    };
    (disposition, detail_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_detail_backpressure_and_authorization_are_protocol() {
        for code in [
            ErrorCode::Backpressure,
            ErrorCode::PermissionDenied,
            ErrorCode::StaleIdentity,
            ErrorCode::InvalidExecution,
        ] {
            assert_eq!(
                classify_v2_incoming_error(code as u16, 0, 7, 0),
                (V2IncomingDisposition::Protocol, 0)
            );
        }
        assert_eq!(
            classify_v2_incoming_error(ErrorCode::MalformedPayload as u16, 0, 7, 0),
            (V2IncomingDisposition::Fatal(ErrorCode::MalformedPayload), 0)
        );
    }

    #[test]
    fn in_range_readable_rejections_keep_the_connection() {
        assert_eq!(
            classify_v2_incoming_error(ErrorCode::Backpressure as u16, 7, 7, 0),
            (V2IncomingDisposition::ClientBackpressure, 7)
        );
        assert_eq!(
            classify_v2_incoming_error(ErrorCode::MalformedPayload as u16, 4, 8, 0),
            (V2IncomingDisposition::ClientBackpressure, 4)
        );
        assert_eq!(
            classify_v2_incoming_error(ErrorCode::PermissionDenied as u16, 5, 8, 0),
            (V2IncomingDisposition::LostController, 5)
        );
        assert_eq!(
            classify_v2_incoming_error(ErrorCode::StaleIdentity as u16, 6, 8, 5),
            (V2IncomingDisposition::LostController, 6)
        );
        assert_eq!(
            classify_v2_incoming_error(ErrorCode::InvalidExecution as u16, 8, 8, 6),
            (V2IncomingDisposition::LostController, 8)
        );
    }

    #[test]
    fn out_of_range_duplicate_and_unsent_ids_are_protocol() {
        assert_eq!(
            classify_v2_incoming_error(ErrorCode::Backpressure as u16, 7, 7, 7),
            (V2IncomingDisposition::Protocol, 7)
        );
        assert_eq!(
            classify_v2_incoming_error(ErrorCode::PermissionDenied as u16, 3, 7, 7),
            (V2IncomingDisposition::Protocol, 7)
        );
        assert_eq!(
            classify_v2_incoming_error(ErrorCode::Backpressure as u16, 9, 7, 0),
            (V2IncomingDisposition::Protocol, 0)
        );
        assert_eq!(
            classify_v2_incoming_error(ErrorCode::Backpressure as u16, 7, 0, 0),
            (V2IncomingDisposition::Protocol, 0),
            "an ID that is not yet wire-complete is outside the sent range"
        );
    }
}
