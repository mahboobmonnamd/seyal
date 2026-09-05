#[derive(Clone, Debug, PartialEq, Eq)]
enum Payload {
    Stored(Vec<u8>),
    Overflow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BoundedCluster {
    payload: Payload,
    committed_width: u8,
    max_payload_bytes: usize,
    overflow_events: usize,
}

impl BoundedCluster {
    fn new(max_payload_bytes: usize) -> Self {
        assert!(max_payload_bytes > 0);
        Self {
            payload: Payload::Stored(Vec::new()),
            committed_width: 0,
            max_payload_bytes,
            overflow_events: 0,
        }
    }

    fn start(&mut self, text: &str, width: u8) {
        self.committed_width = width;
        self.payload = if text.len() <= self.max_payload_bytes {
            Payload::Stored(text.as_bytes().to_vec())
        } else {
            self.overflow_events += 1;
            Payload::Overflow
        };
    }

    /// The caller has already established, using incremental grapheme-break
    /// state, that `text` extends the active cluster. This probe deliberately
    /// does not make segmentation depend on stored payload bytes.
    fn append_same_cluster(&mut self, text: &str, natural_width: u8) {
        self.committed_width = self.committed_width.max(natural_width);
        let Payload::Stored(bytes) = &mut self.payload else {
            return;
        };

        if bytes.len().saturating_add(text.len()) > self.max_payload_bytes {
            self.payload = Payload::Overflow;
            self.overflow_events += 1;
            return;
        }
        bytes.extend_from_slice(text.as_bytes());
    }

    fn retained_payload_bytes(&self) -> usize {
        match &self.payload {
            Payload::Stored(bytes) => bytes.len(),
            Payload::Overflow => 0,
        }
    }

    fn selected_text(&self) -> &str {
        match &self.payload {
            Payload::Stored(bytes) => std::str::from_utf8(bytes).expect("probe stores valid UTF-8"),
            Payload::Overflow => "\u{fffd}",
        }
    }

    fn is_overflow(&self) -> bool {
        matches!(self.payload, Payload::Overflow)
    }
}

pub(crate) fn report_overflow_policy() {
    let limit = 256usize;
    let mut cluster = BoundedCluster::new(limit);
    cluster.start("a", 1);
    for _ in 0..4_096 {
        cluster.append_same_cluster("\u{301}", 1);
    }

    println!(
        "RESOURCE\tlimit_bytes={}\tinput_bytes={}\tretained_payload_bytes={}\tcommitted_width={}\toverflow={}\toverflow_events={}\tselection={:?}",
        limit,
        1 + 4_096 * "\u{301}".len(),
        cluster.retained_payload_bytes(),
        cluster.committed_width,
        cluster.is_overflow(),
        cluster.overflow_events,
        cluster.selected_text(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pathological_cluster_drops_payload_at_bound_instead_of_growing_memory() {
        let mut cluster = BoundedCluster::new(256);
        cluster.start("a", 1);
        for _ in 0..4_096 {
            cluster.append_same_cluster("\u{301}", 1);
        }
        assert!(cluster.is_overflow());
        assert_eq!(cluster.retained_payload_bytes(), 0);
        assert_eq!(cluster.committed_width, 1);
        assert_eq!(cluster.overflow_events, 1);
        assert_eq!(cluster.selected_text(), "\u{fffd}");
    }

    #[test]
    fn overflow_preserves_already_committed_terminal_occupation_width() {
        let mut cluster = BoundedCluster::new(8);
        cluster.start("👩", 2);
        for _ in 0..32 {
            cluster.append_same_cluster("\u{301}", 2);
        }
        assert!(cluster.is_overflow());
        assert_eq!(cluster.committed_width, 2);
    }

    #[test]
    fn next_grapheme_recovers_normally_after_overflowed_cluster() {
        let mut cluster = BoundedCluster::new(4);
        cluster.start("a", 1);
        cluster.append_same_cluster("\u{301}\u{301}", 1);
        assert!(cluster.is_overflow());

        cluster.start("B", 1);
        assert!(!cluster.is_overflow());
        assert_eq!(cluster.selected_text(), "B");
        assert_eq!(cluster.committed_width, 1);
        assert_eq!(cluster.overflow_events, 1);
    }
}
