#[derive(Debug, Default)]
struct AppendOnlyStore {
    bytes: Vec<u8>,
    current_offset: usize,
    current_len: usize,
}

impl AppendOnlyStore {
    fn overwrite(&mut self, text: &str) {
        self.current_offset = self.bytes.len();
        self.current_len = text.len();
        self.bytes.extend_from_slice(text.as_bytes());
    }

    fn live_bytes(&self) -> usize {
        self.current_len
    }

    fn allocated_payload_bytes(&self) -> usize {
        self.bytes.len()
    }
}

#[derive(Debug, Default)]
struct ReusableSlot {
    bytes: Vec<u8>,
}

impl ReusableSlot {
    fn overwrite(&mut self, text: &str) {
        self.bytes.clear();
        self.bytes.extend_from_slice(text.as_bytes());
    }

    fn live_bytes(&self) -> usize {
        self.bytes.len()
    }

    fn capacity_bytes(&self) -> usize {
        self.bytes.capacity()
    }
}

pub(crate) fn report_storage_pressure() {
    const VALUES: &[&str] = &["A", "e\u{301}", "界", "👨‍👩‍👧‍👦"];
    const OVERWRITES: usize = 100_000;

    let mut append_only = AppendOnlyStore::default();
    let mut reusable = ReusableSlot::default();

    for index in 0..OVERWRITES {
        let value = VALUES[index % VALUES.len()];
        append_only.overwrite(value);
        reusable.overwrite(value);
    }

    println!(
        "STORAGE\toverwrites={OVERWRITES}\tappend_payload={}\tappend_live={}\treusable_capacity={}\treusable_live={}",
        append_only.allocated_payload_bytes(),
        append_only.live_bytes(),
        reusable.capacity_bytes(),
        reusable.live_bytes(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_only_storage_grows_with_overwrites() {
        let mut store = AppendOnlyStore::default();
        for _ in 0..1_000 {
            store.overwrite("👨‍👩‍👧‍👦");
        }
        assert_eq!(store.live_bytes(), "👨‍👩‍👧‍👦".len());
        assert_eq!(
            store.allocated_payload_bytes(),
            1_000 * "👨‍👩‍👧‍👦".len()
        );
    }

    #[test]
    fn reusable_slot_stays_bounded_by_largest_seen_cluster() {
        let mut slot = ReusableSlot::default();
        for _ in 0..1_000 {
            slot.overwrite("A");
            slot.overwrite("👨‍👩‍👧‍👦");
            slot.overwrite("e\u{301}");
        }
        assert_eq!(slot.live_bytes(), "e\u{301}".len());
        assert!(slot.capacity_bytes() >= "👨‍👩‍👧‍👦".len());
        assert!(slot.capacity_bytes() < 1_000 * "👨‍👩‍👧‍👦".len());
    }
}
