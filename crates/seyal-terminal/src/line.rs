use crate::error::TerminalError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineId(pub u64);

#[derive(Clone, Copy, Debug)]
pub(crate) struct LineIdAllocator {
    next: Option<u64>,
}

impl LineIdAllocator {
    pub(crate) fn new() -> Self {
        Self { next: Some(1) }
    }

    pub(crate) fn can_allocate(&self, count: usize) -> bool {
        if count == 0 {
            return true;
        }
        let Some(next) = self.next else {
            return false;
        };
        let Ok(offset) = u64::try_from(count - 1) else {
            return false;
        };
        next.checked_add(offset).is_some()
    }

    pub(crate) fn allocate(&mut self) -> Result<LineId, TerminalError> {
        let current = self.next.ok_or(TerminalError::LineIdentityExhausted)?;
        self.next = current.checked_add(1);
        Ok(LineId(current))
    }

    #[cfg(test)]
    pub(crate) fn with_next(next: Option<u64>) -> Self {
        Self { next }
    }
}

impl Default for LineIdAllocator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_issues_last_id_once_then_fails_explicitly() {
        let mut allocator = LineIdAllocator::with_next(Some(u64::MAX - 1));

        assert_eq!(allocator.allocate(), Ok(LineId(u64::MAX - 1)));
        assert_eq!(allocator.allocate(), Ok(LineId(u64::MAX)));
        assert_eq!(
            allocator.allocate(),
            Err(TerminalError::LineIdentityExhausted)
        );
    }

    #[test]
    fn allocation_capacity_check_never_wraps() {
        let allocator = LineIdAllocator::with_next(Some(u64::MAX));

        assert!(allocator.can_allocate(1));
        assert!(!allocator.can_allocate(2));
    }
}
