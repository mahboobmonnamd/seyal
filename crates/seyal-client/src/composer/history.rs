//! Pane-local composer history and fuzzy recall (#933).
//!
//! History is product state owned by the Pane composer: a bounded list of
//! commands the Runtime accepted from this Pane's composer. It is not the
//! Block `history_range`, not shell history, and never shared across Panes.
//! Ranking is synchronous over at most [`HISTORY_CAPACITY`] entries and runs
//! only on the product-state path, never on PTY/VT/damage/render paths.

use std::collections::VecDeque;

/// Upper bound of retained entries per Pane. Oldest entries are evicted.
pub const HISTORY_CAPACITY: usize = 200;

/// Maximum rows projected to the host for one filter result.
pub const HISTORY_VISIBLE_ROWS: usize = 8;

/// One accepted composer submission. `seq` is a Pane-local recency ordinal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub command: String,
    pub seq: u64,
}

/// Bounded, recency-ordered command history for one Pane.
#[derive(Clone, Debug, Default)]
pub struct PaneHistory {
    entries: VecDeque<HistoryEntry>,
    next_seq: u64,
}

impl PaneHistory {
    /// Record an accepted submission. A command equal to the most recent
    /// entry only refreshes its recency instead of duplicating it.
    pub fn record(&mut self, command: &str) {
        let command = command.trim_end();
        if command.is_empty() {
            return;
        }
        self.next_seq = self.next_seq.saturating_add(1);
        if let Some(back) = self.entries.back_mut()
            && back.command == command
        {
            back.seq = self.next_seq;
            return;
        }
        if self.entries.len() == HISTORY_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(HistoryEntry {
            command: command.to_owned(),
            seq: self.next_seq,
        });
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Rank entries for `query`, most relevant first, bounded to
    /// [`HISTORY_VISIBLE_ROWS`]. An empty query returns the most recent
    /// entries in recency order.
    pub fn rank(&self, query: &str) -> Vec<&HistoryEntry> {
        let mut scored: Vec<(MatchClass, &HistoryEntry)> = self
            .entries
            .iter()
            .filter_map(|entry| classify(query, &entry.command).map(|class| (class, entry)))
            .collect();
        scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.seq.cmp(&a.1.seq)));
        scored
            .into_iter()
            .take(HISTORY_VISIBLE_ROWS)
            .map(|(_, entry)| entry)
            .collect()
    }
}

/// Match strength. Lower sorts first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MatchClass {
    Empty,
    Prefix,
    TokenPrefix,
    Subsequence,
}

fn classify(query: &str, command: &str) -> Option<MatchClass> {
    if query.is_empty() {
        return Some(MatchClass::Empty);
    }
    let query_lower = query.to_lowercase();
    let command_lower = command.to_lowercase();
    if command_lower.starts_with(&query_lower) {
        return Some(MatchClass::Prefix);
    }
    if command_lower
        .split_whitespace()
        .any(|token| token.starts_with(&query_lower))
    {
        return Some(MatchClass::TokenPrefix);
    }
    if is_subsequence(&query_lower, &command_lower) {
        return Some(MatchClass::Subsequence);
    }
    None
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = needle.chars().filter(|c| !c.is_whitespace());
    let mut current = chars.next();
    for candidate in haystack.chars() {
        match current {
            None => return true,
            Some(wanted) if wanted == candidate => current = chars.next(),
            Some(_) => {}
        }
    }
    current.is_none()
}

/// Open helper surface anchored above one Pane composer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryOverlay {
    pub query: String,
    pub selected: usize,
}

impl HistoryOverlay {
    /// Clamp the selection to the current result count. Zero rows keep
    /// selection at zero so a later non-empty result starts at the top.
    pub fn clamp(&mut self, row_count: usize) {
        if row_count == 0 {
            self.selected = 0;
        } else if self.selected >= row_count {
            self.selected = row_count - 1;
        }
    }

    pub fn step(&mut self, delta: i32, row_count: usize) {
        if row_count == 0 {
            self.selected = 0;
            return;
        }
        let last = (row_count - 1) as i64;
        let next = (self.selected as i64 + delta as i64).clamp(0, last);
        self.selected = next as usize;
    }
}

/// Read-only host projection of an open overlay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryOverlaySnapshot {
    pub query: String,
    pub rows: Vec<String>,
    pub selected: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(commands: &[&str]) -> PaneHistory {
        let mut history = PaneHistory::default();
        for command in commands {
            history.record(command);
        }
        history
    }

    fn commands(entries: Vec<&HistoryEntry>) -> Vec<&str> {
        entries.into_iter().map(|e| e.command.as_str()).collect()
    }

    #[test]
    fn empty_query_returns_most_recent_first() {
        let history = history(&["git status", "ls -la", "cargo test"]);
        assert_eq!(
            commands(history.rank("")),
            vec!["cargo test", "ls -la", "git status"]
        );
    }

    #[test]
    fn consecutive_duplicate_refreshes_recency_without_duplicating() {
        let repeated = history(&["ls", "git status", "git status"]);
        assert_eq!(repeated.len(), 2);
        assert_eq!(commands(repeated.rank("")), vec!["git status", "ls"]);
        let bumped = history(&["git status", "ls", "git status"]);
        assert_eq!(bumped.len(), 3);
    }

    #[test]
    fn blank_and_trailing_newline_commands_are_normalized() {
        let history = history(&["", "   ", "echo hi\r", "echo hi\n"]);
        assert_eq!(history.len(), 1);
        assert_eq!(commands(history.rank("")), vec!["echo hi"]);
    }

    #[test]
    fn capacity_evicts_oldest() {
        let mut history = PaneHistory::default();
        for index in 0..(HISTORY_CAPACITY + 5) {
            history.record(&format!("cmd {index}"));
        }
        assert_eq!(history.len(), HISTORY_CAPACITY);
        let all = history.rank("cmd");
        assert_eq!(all[0].command, format!("cmd {}", HISTORY_CAPACITY + 4));
        assert!(history.rank("cmd 0").iter().all(|e| e.command != "cmd 0"));
    }

    #[test]
    fn ranking_prefers_prefix_then_token_then_subsequence() {
        let history = history(&[
            "cat hello",
            "make check",
            "cargo check",
            "ck",
            "chk-lint",
            "check-all",
        ]);
        assert_eq!(
            commands(history.rank("ch")),
            vec![
                "check-all",
                "chk-lint",
                "cargo check",
                "make check",
                "cat hello"
            ]
        );
    }

    #[test]
    fn ranking_is_case_insensitive_and_bounded() {
        let mut history = PaneHistory::default();
        for index in 0..20 {
            history.record(&format!("Echo {index}"));
        }
        let rows = history.rank("echo");
        assert_eq!(rows.len(), HISTORY_VISIBLE_ROWS);
        assert_eq!(rows[0].command, "Echo 19");
    }

    #[test]
    fn non_matching_query_yields_no_rows() {
        let history = history(&["git status", "ls"]);
        assert!(history.rank("zzz").is_empty());
        assert!(history.rank("s l").is_empty());
    }

    #[test]
    fn subsequence_ignores_query_whitespace() {
        let history = history(&["git commit -m fix"]);
        assert_eq!(commands(history.rank("gc m")), vec!["git commit -m fix"]);
    }

    #[test]
    fn overlay_selection_clamps_and_steps() {
        let mut overlay = HistoryOverlay::default();
        overlay.step(1, 3);
        overlay.step(1, 3);
        overlay.step(1, 3);
        assert_eq!(overlay.selected, 2);
        overlay.step(-5, 3);
        assert_eq!(overlay.selected, 0);
        overlay.selected = 7;
        overlay.clamp(3);
        assert_eq!(overlay.selected, 2);
        overlay.clamp(0);
        assert_eq!(overlay.selected, 0);
        overlay.step(1, 0);
        assert_eq!(overlay.selected, 0);
    }
}
