//! Pure Candidate-D recovery-request coalescing seam.
//!
//! Runtime uses this exact helper for explicit Resync and continuity-loss
//! recovery scheduling. Keeping the deduplication rule pure lets fuzzing
//! exercise production behavior without creating PTYs or sockets per input.

use std::collections::{HashSet, VecDeque};

/// Records one pending current-snapshot requirement for `token`.
///
/// Returns `true` only when this call created a new pending requirement.
/// Repeated explicit Resync requests and repeated generation-gap recovery
/// requests for the same connection therefore collapse to one queued token.
#[doc(hidden)]
pub fn schedule_snapshot_recovery(
    queue: &mut VecDeque<u64>,
    pending: &mut HashSet<u64>,
    token: u64,
) -> bool {
    if !pending.insert(token) {
        return false;
    }
    queue.push_back(token);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_requests_coalesce_to_one_queue_entry() {
        let mut queue = VecDeque::new();
        let mut pending = HashSet::new();

        assert!(schedule_snapshot_recovery(&mut queue, &mut pending, 7));
        assert!(!schedule_snapshot_recovery(&mut queue, &mut pending, 7));
        assert!(!schedule_snapshot_recovery(&mut queue, &mut pending, 7));
        assert_eq!(queue.into_iter().collect::<Vec<_>>(), vec![7]);
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn independent_connections_remain_independent() {
        let mut queue = VecDeque::new();
        let mut pending = HashSet::new();

        assert!(schedule_snapshot_recovery(&mut queue, &mut pending, 1));
        assert!(schedule_snapshot_recovery(&mut queue, &mut pending, 2));
        assert!(!schedule_snapshot_recovery(&mut queue, &mut pending, 1));
        assert_eq!(queue.into_iter().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(pending.len(), 2);
    }
}
