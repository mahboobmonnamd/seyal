#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineId(pub u64);

#[derive(Clone, Copy, Debug)]
pub(crate) struct LineClock {
    namespace: u32,
    next: u32,
}

impl LineClock {
    pub(crate) fn new(namespace: u32) -> Self {
        Self { namespace, next: 1 }
    }

    pub(crate) fn allocate(&mut self) -> LineId {
        let local = self.next;
        self.next = self.next.saturating_add(1);
        LineId((u64::from(self.namespace) << 32) | u64::from(local))
    }
}
