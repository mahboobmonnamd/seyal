#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Damage {
    pub generation: u64,
    pub full: bool,
    pub first_row: u16,
    pub last_row: u16,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Mutation {
    full: bool,
    first_row: Option<u16>,
    last_row: u16,
}

impl Mutation {
    pub(crate) fn none() -> Self {
        Self::default()
    }

    pub(crate) fn row(row: u16) -> Self {
        Self {
            full: false,
            first_row: Some(row),
            last_row: row,
        }
    }

    pub(crate) fn rows(first_row: u16, last_row: u16) -> Self {
        Self {
            full: false,
            first_row: Some(first_row.min(last_row)),
            last_row: first_row.max(last_row),
        }
    }

    pub(crate) fn full(rows: u16) -> Self {
        Self {
            full: true,
            first_row: Some(0),
            last_row: rows.saturating_sub(1),
        }
    }

    pub(crate) fn merge(self, other: Self) -> Self {
        match (self.first_row, other.first_row) {
            (None, _) => other,
            (_, None) => self,
            (Some(a), Some(b)) => Self {
                full: self.full || other.full,
                first_row: Some(a.min(b)),
                last_row: self.last_row.max(other.last_row),
            },
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct DamageTracker {
    generation: u64,
    dirty: Mutation,
    published: Option<Damage>,
}

impl DamageTracker {
    pub(crate) fn mark(&mut self, mutation: Mutation) {
        self.dirty = self.dirty.merge(mutation);
    }

    pub(crate) fn commit(&mut self) {
        let Some(first_row) = self.dirty.first_row else {
            return;
        };

        self.generation = self.generation.saturating_add(1);
        let next = Damage {
            generation: self.generation,
            full: self.dirty.full,
            first_row,
            last_row: self.dirty.last_row,
        };
        self.dirty = Mutation::none();

        self.published = Some(match self.published {
            None => next,
            Some(previous) => Damage {
                generation: next.generation,
                full: previous.full || next.full,
                first_row: previous.first_row.min(next.first_row),
                last_row: previous.last_row.max(next.last_row),
            },
        });
    }

    pub(crate) fn take(&mut self) -> Option<Damage> {
        self.published.take()
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
}
