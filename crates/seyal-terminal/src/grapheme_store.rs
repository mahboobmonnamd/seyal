//! Bounded reclaimable variable grapheme payload storage for `TerminalState`.

#![allow(dead_code)]

#[cfg(test)]
use std::collections::HashSet;

/// Maximum UTF-8 bytes retained for one active canonical grapheme (SPEC-011).
pub const MAX_ACTIVE_GRAPHEME_BYTES: usize = 8_192;

/// Maximum live variable payload bytes per `TerminalState` (SPEC-011).
pub const MAX_LIVE_VARIABLE_BYTES: usize = 2_097_152;

/// Sentinel store id meaning the lead uses only the inline scalar.
pub const INLINE_STORE_ID: u32 = u32::MAX;

#[derive(Debug, Default)]
pub(crate) struct GraphemeStore {
    entries: Vec<Option<Vec<u8>>>,
    free: Vec<u32>,
    live_bytes: usize,
    pub grapheme_payload_overflow_count: u64,
    pub grapheme_store_capacity_fallback_count: u64,
}

impl GraphemeStore {
    pub(crate) fn live_bytes(&self) -> usize {
        self.live_bytes
    }

    pub(crate) fn get(&self, id: u32) -> Option<&[u8]> {
        if id == INLINE_STORE_ID {
            return None;
        }
        self.entries
            .get(id as usize)
            .and_then(|entry| entry.as_deref())
    }

    pub(crate) fn release(&mut self, id: u32) {
        if id == INLINE_STORE_ID {
            return;
        }
        let Some(slot) = self.entries.get_mut(id as usize) else {
            return;
        };
        if let Some(bytes) = slot.take() {
            self.live_bytes = self.live_bytes.saturating_sub(bytes.len());
            self.free.push(id);
        }
    }

    /// Inserts UTF-8 payload, reclaiming `release_first` before measuring the
    /// live-store budget. Returns `INLINE_STORE_ID` with overflow/fallback
    /// counters updated when the payload cannot be retained.
    pub(crate) fn insert_replacing(
        &mut self,
        payload: &[u8],
        release_first: Option<u32>,
    ) -> StoreAdmit {
        if let Some(id) = release_first {
            self.release(id);
        }
        if payload.is_empty() {
            return StoreAdmit::Inline;
        }
        if payload.len() > MAX_ACTIVE_GRAPHEME_BYTES {
            self.grapheme_payload_overflow_count =
                self.grapheme_payload_overflow_count.saturating_add(1);
            return StoreAdmit::OverflowSentinel;
        }
        if self.live_bytes.saturating_add(payload.len()) > MAX_LIVE_VARIABLE_BYTES {
            self.grapheme_store_capacity_fallback_count = self
                .grapheme_store_capacity_fallback_count
                .saturating_add(1);
            return StoreAdmit::OverflowSentinel;
        }

        let id = if let Some(id) = self.free.pop() {
            self.entries[id as usize] = Some(payload.to_vec());
            id
        } else {
            let id = self.entries.len() as u32;
            self.entries.push(Some(payload.to_vec()));
            id
        };
        self.live_bytes = self.live_bytes.saturating_add(payload.len());
        StoreAdmit::Stored(id)
    }

    /// Compacts dead slots already released; no-op placeholder for future arena
    /// densification. Visible live payload is never evicted.
    pub(crate) fn reclaim_dead(&mut self) {
        let _ = self.free.len();
    }

    #[cfg(test)]
    pub(crate) fn occupied_ids(&self) -> HashSet<u32> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.as_ref().map(|_| index as u32))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoreAdmit {
    Inline,
    Stored(u32),
    OverflowSentinel,
}
