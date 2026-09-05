use unicode_segmentation::UnicodeSegmentation;

const MAX_UTF8: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Print(char),
    Execute(u8),
    Malformed,
}

#[derive(Debug, Default)]
struct GroundDecoder {
    utf8: [u8; MAX_UTF8],
    utf8_len: u8,
    utf8_needed: u8,
    putback: Option<u8>,
}

impl GroundDecoder {
    fn feed(&mut self, bytes: &[u8], events: &mut Vec<Event>) {
        let mut index = 0usize;
        while index < bytes.len() || self.putback.is_some() {
            let byte = match self.putback.take() {
                Some(byte) => byte,
                None => {
                    let byte = bytes[index];
                    index += 1;
                    byte
                }
            };
            self.step(byte, events);
        }
    }

    fn finish(&mut self, events: &mut Vec<Event>) {
        if self.utf8_needed != 0 {
            self.abort_utf8(events);
        }
        self.putback = None;
    }

    fn step(&mut self, byte: u8, events: &mut Vec<Event>) {
        if self.utf8_needed != 0 {
            if (0x80..=0xbf).contains(&byte) {
                self.utf8[usize::from(self.utf8_len)] = byte;
                self.utf8_len += 1;
                if self.utf8_len == self.utf8_needed {
                    self.finish_utf8(events);
                }
                return;
            }
            self.abort_utf8(events);
            self.putback = Some(byte);
            return;
        }

        match byte {
            0x00..=0x1f => events.push(Event::Execute(byte)),
            0x20..=0x7e => events.push(Event::Print(char::from(byte))),
            0x7f => {}
            0xc2..=0xdf => self.start_utf8(byte, 2),
            0xe0..=0xef => self.start_utf8(byte, 3),
            0xf0..=0xf4 => self.start_utf8(byte, 4),
            _ => self.replacement(events),
        }
    }

    fn start_utf8(&mut self, lead: u8, needed: u8) {
        self.utf8 = [0; MAX_UTF8];
        self.utf8[0] = lead;
        self.utf8_len = 1;
        self.utf8_needed = needed;
    }

    fn finish_utf8(&mut self, events: &mut Vec<Event>) {
        let len = usize::from(self.utf8_needed);
        let decoded = std::str::from_utf8(&self.utf8[..len])
            .ok()
            .and_then(|text| text.chars().next());
        self.utf8_len = 0;
        self.utf8_needed = 0;
        match decoded {
            Some(character) => events.push(Event::Print(character)),
            None => self.replacement(events),
        }
    }

    fn abort_utf8(&mut self, events: &mut Vec<Event>) {
        self.utf8_len = 0;
        self.utf8_needed = 0;
        self.replacement(events);
    }

    fn replacement(&self, events: &mut Vec<Event>) {
        events.push(Event::Malformed);
        events.push(Event::Print('\u{fffd}'));
    }
}

fn decode_with_chunks(input: &[u8], chunk_sizes: &[usize], finish: bool) -> Vec<Event> {
    let mut decoder = GroundDecoder::default();
    let mut events = Vec::new();
    let mut offset = 0usize;
    for &size in chunk_sizes {
        let end = offset.saturating_add(size).min(input.len());
        decoder.feed(&input[offset..end], &mut events);
        offset = end;
    }
    if offset < input.len() {
        decoder.feed(&input[offset..], &mut events);
    }
    if finish {
        decoder.finish(&mut events);
    }
    events
}

fn printable_text(events: &[Event]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Print(character) => Some(*character),
            Event::Execute(_) | Event::Malformed => None,
        })
        .collect()
}

pub(crate) fn report_transport_semantics() {
    let zwj = "👩‍💻";
    let bytes = zwj.as_bytes();
    let bytewise = decode_with_chunks(bytes, &vec![1; bytes.len()], true);
    let text = printable_text(&bytewise);
    println!(
        "TRANSPORT\tbytewise-emoji-zwj\tbytes={}\tprints={}\tgraphemes={}\tmalformed={}",
        bytes.len(),
        text.chars().count(),
        text.graphemes(true).count(),
        bytewise
            .iter()
            .filter(|event| matches!(event, Event::Malformed))
            .count(),
    );

    let malformed = decode_with_chunks(&[0xe2, b'A'], &[1, 1], true);
    println!(
        "TRANSPORT\tmalformed-then-ascii\tevents={malformed:?}\ttext={:?}",
        printable_text(&malformed)
    );

    let mut decoder = GroundDecoder::default();
    let mut events = Vec::new();
    decoder.feed(b"e", &mut events);
    decoder.feed(&[0x07], &mut events);
    decoder.feed("\u{301}".as_bytes(), &mut events);
    decoder.finish(&mut events);
    println!("TRANSPORT\tcontrol-between-scalars\tevents={events:?}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected_prints(text: &str) -> Vec<Event> {
        text.chars().map(Event::Print).collect()
    }

    #[test]
    fn every_two_chunk_split_of_four_byte_scalar_is_invariant() {
        let text = "👩";
        let bytes = text.as_bytes();
        let expected = expected_prints(text);
        for split in 0..=bytes.len() {
            let actual = decode_with_chunks(bytes, &[split], true);
            assert_eq!(actual, expected, "split at byte {split}");
        }
    }

    #[test]
    fn bytewise_feed_preserves_multi_scalar_grapheme_without_raw_pty_buffering() {
        let text = "👩‍💻";
        let events = decode_with_chunks(text.as_bytes(), &vec![1; text.len()], true);
        assert_eq!(events, expected_prints(text));
        let printable = printable_text(&events);
        assert_eq!(printable.graphemes(true).count(), 1);
    }

    #[test]
    fn malformed_sequence_reprocesses_following_ascii() {
        let events = decode_with_chunks(&[0xe2, b'A'], &[1, 1], true);
        assert_eq!(
            events,
            vec![Event::Malformed, Event::Print('\u{fffd}'), Event::Print('A')]
        );
    }

    #[test]
    fn truncated_sequence_only_becomes_replacement_on_finish() {
        let mut decoder = GroundDecoder::default();
        let mut events = Vec::new();
        decoder.feed(&[0xf0, 0x9f], &mut events);
        assert!(events.is_empty());
        decoder.finish(&mut events);
        assert_eq!(events, vec![Event::Malformed, Event::Print('\u{fffd}')]);
    }

    #[test]
    fn control_event_remains_distinct_from_unicode_scalar_stream() {
        let mut decoder = GroundDecoder::default();
        let mut events = Vec::new();
        decoder.feed(b"e", &mut events);
        decoder.feed(&[0x07], &mut events);
        decoder.feed("\u{301}".as_bytes(), &mut events);
        decoder.finish(&mut events);
        assert_eq!(
            events,
            vec![Event::Print('e'), Event::Execute(0x07), Event::Print('\u{301}')]
        );
    }
}
