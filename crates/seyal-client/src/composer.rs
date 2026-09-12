//! Portable composer draft, submission fencing, and Block presentation.
//!
//! This module owns per-Pane draft lifecycle, available/busy/hidden eligibility,
//! request-id correlation, and a read-only projection of Runtime Block metadata.
//! It is not a BlockTimeline, PTY, VT/grid, or renderer. Hosts dispatch
//! [`ComposerAction`] and render [`ComposerSnapshot`]. Do not call this from
//! the PTY→VT→damage path. Do not invent Block completions.

use std::collections::HashMap;
use std::fmt;

use seyal_core::{BlockId, PaneId};

use crate::presentation::{InputRoute, PresentationMode};

/// Host-visible composer eligibility for one Pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposerMode {
    Available,
    Busy { process: String },
    Hidden,
}

/// Why a [`ComposerAction`] was rejected. The previous state is unchanged
/// unless the action is a matched result that only clears correlation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposerError {
    UnknownPane,
    EmptyDraft,
    SubmitDisabled,
    StaleRequest,
    StaleEpoch,
}

impl ComposerError {
    fn message(self) -> &'static str {
        match self {
            Self::UnknownPane => "Unknown Pane.",
            Self::EmptyDraft => "Composer draft is empty.",
            Self::SubmitDisabled => "Composer submit is unavailable.",
            Self::StaleRequest => "Composer result does not match the pending request.",
            Self::StaleEpoch => "Composer action epoch is stale.",
        }
    }
}

impl fmt::Display for ComposerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

/// Projected Block lifecycle. Runtime remains the writer of these facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockPresentationState {
    Running,
    Completed,
    Failed,
}

/// Canonical Runtime/Workspace Block metadata consumed by the projection.
/// This type does not create, complete, or own Blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeBlockRecord {
    pub id: BlockId,
    pub command: String,
    pub start_line: u64,
    pub end_line: Option<u64>,
    pub running: bool,
    pub exit_status: Option<i32>,
}

impl RuntimeBlockRecord {
    fn presentation_state(&self) -> BlockPresentationState {
        if self.running {
            BlockPresentationState::Running
        } else if self.exit_status.unwrap_or(1) == 0 {
            BlockPresentationState::Completed
        } else {
            BlockPresentationState::Failed
        }
    }
}

/// Pane-qualified Block presentation for hosts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockProjection {
    pub pane: PaneId,
    pub id: BlockId,
    pub command: String,
    pub state: BlockPresentationState,
    pub start_line: u64,
    pub end_line: Option<u64>,
}

/// Typed host → Rust command. One action is one coarse transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposerAction {
    EnsurePane {
        pane: PaneId,
    },
    SetDraft {
        pane: PaneId,
        text: String,
        epoch: u64,
    },
    Submit {
        pane: PaneId,
        epoch: u64,
    },
    ApplyResult {
        pane: PaneId,
        request_id: u64,
        accepted: bool,
    },
    SetBusy {
        pane: PaneId,
        process: Option<String>,
    },
    ApplyPresentation {
        pane: PaneId,
        mode: PresentationMode,
        input_route: InputRoute,
    },
    ApplyRuntimeBlocks {
        pane: PaneId,
        records: Vec<RuntimeBlockRecord>,
    },
}

/// Read-only projection for native hosts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposerSnapshot {
    pub pane: PaneId,
    pub draft: String,
    pub mode: ComposerMode,
    pub can_submit: bool,
    pub pending_request_id: Option<u64>,
    pub epoch: u64,
    pub allows_direct_terminal: bool,
    pub blocks: Vec<BlockProjection>,
    pub last_error: Option<ComposerError>,
}

#[derive(Clone, Debug)]
struct PaneComposer {
    draft: String,
    pending_request_id: Option<u64>,
    next_request_id: u64,
    epoch: u64,
    busy_process: Option<String>,
    presentation_mode: PresentationMode,
    input_route: InputRoute,
    blocks: Vec<BlockProjection>,
}

impl PaneComposer {
    fn new() -> Self {
        Self {
            draft: String::new(),
            pending_request_id: None,
            next_request_id: 1,
            epoch: 1,
            busy_process: None,
            presentation_mode: PresentationMode::Flow,
            input_route: InputRoute::Composer,
            blocks: Vec::new(),
        }
    }

    fn bump_epoch(&mut self) {
        self.epoch = self.epoch.saturating_add(1);
    }

    fn require_epoch(&self, epoch: u64) -> Result<(), ComposerError> {
        if self.epoch == epoch {
            Ok(())
        } else {
            Err(ComposerError::StaleEpoch)
        }
    }

    fn mode(&self) -> ComposerMode {
        if self.input_route != InputRoute::Composer
            || self.presentation_mode != PresentationMode::Flow
        {
            return ComposerMode::Hidden;
        }
        if let Some(process) = &self.busy_process {
            return ComposerMode::Busy {
                process: process.clone(),
            };
        }
        if self.pending_request_id.is_some() {
            return ComposerMode::Busy {
                process: self.draft.clone(),
            };
        }
        ComposerMode::Available
    }

    fn can_submit(&self) -> bool {
        matches!(self.mode(), ComposerMode::Available) && !self.draft.is_empty()
    }

    fn allocate_request_id(&mut self) -> u64 {
        let request_id = self.next_request_id;
        self.next_request_id = if request_id == u64::MAX {
            1
        } else {
            request_id + 1
        };
        request_id
    }

    fn project_blocks(&self, pane: PaneId, records: &[RuntimeBlockRecord]) -> Vec<BlockProjection> {
        records
            .iter()
            .map(|record| BlockProjection {
                pane,
                id: record.id,
                command: record.command.clone(),
                state: record.presentation_state(),
                start_line: record.start_line,
                end_line: record.end_line,
            })
            .collect()
    }
}

/// Authoritative headed composer/Block-projection state.
#[derive(Clone, Debug, Default)]
pub struct ComposerState {
    panes: HashMap<PaneId, PaneComposer>,
    last_error: Option<ComposerError>,
}

impl ComposerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, action: ComposerAction) -> Result<Option<u64>, ComposerError> {
        self.last_error = None;
        match action {
            ComposerAction::EnsurePane { pane } => {
                self.pane_mut(pane);
                Ok(None)
            }
            ComposerAction::SetDraft { pane, text, epoch } => {
                let composer = self.existing_mut(pane)?;
                composer.require_epoch(epoch)?;
                composer.draft = text;
                Ok(None)
            }
            ComposerAction::Submit { pane, epoch } => {
                let composer = self.existing_mut(pane)?;
                composer.require_epoch(epoch)?;
                if composer.draft.is_empty() {
                    return self.fail(ComposerError::EmptyDraft);
                }
                if !composer.can_submit() {
                    return self.fail(ComposerError::SubmitDisabled);
                }
                let request_id = composer.allocate_request_id();
                composer.pending_request_id = Some(request_id);
                composer.bump_epoch();
                Ok(Some(request_id))
            }
            ComposerAction::ApplyResult {
                pane,
                request_id,
                accepted,
            } => {
                let composer = self.existing_mut(pane)?;
                if composer.pending_request_id != Some(request_id) {
                    return self.fail(ComposerError::StaleRequest);
                }
                composer.pending_request_id = None;
                if accepted {
                    composer.draft.clear();
                }
                composer.bump_epoch();
                Ok(None)
            }
            ComposerAction::SetBusy { pane, process } => {
                let composer = self.pane_mut(pane);
                if composer.busy_process != process {
                    composer.busy_process = process;
                    composer.bump_epoch();
                }
                Ok(None)
            }
            ComposerAction::ApplyPresentation {
                pane,
                mode,
                input_route,
            } => {
                let composer = self.pane_mut(pane);
                if composer.presentation_mode != mode || composer.input_route != input_route {
                    composer.presentation_mode = mode;
                    composer.input_route = input_route;
                    composer.bump_epoch();
                }
                Ok(None)
            }
            ComposerAction::ApplyRuntimeBlocks { pane, records } => {
                let composer = self.pane_mut(pane);
                composer.blocks = composer.project_blocks(pane, &records);
                Ok(None)
            }
        }
    }

    pub fn snapshot(&self, pane: PaneId) -> Result<ComposerSnapshot, ComposerError> {
        let composer = self.panes.get(&pane).ok_or(ComposerError::UnknownPane)?;
        Ok(ComposerSnapshot {
            pane,
            draft: composer.draft.clone(),
            mode: composer.mode(),
            can_submit: composer.can_submit(),
            pending_request_id: composer.pending_request_id,
            epoch: composer.epoch,
            allows_direct_terminal: composer.input_route == InputRoute::DirectTerminal,
            blocks: composer.blocks.clone(),
            last_error: self.last_error,
        })
    }

    fn pane_mut(&mut self, pane: PaneId) -> &mut PaneComposer {
        self.panes.entry(pane).or_insert_with(PaneComposer::new)
    }

    fn existing_mut(&mut self, pane: PaneId) -> Result<&mut PaneComposer, ComposerError> {
        if !self.panes.contains_key(&pane) {
            return self.fail(ComposerError::UnknownPane);
        }
        Ok(self.pane_mut(pane))
    }

    fn fail<T>(&mut self, error: ComposerError) -> Result<T, ComposerError> {
        self.last_error = Some(error);
        Err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane() -> PaneId {
        PaneId::from_bytes([0x11; 16])
    }

    fn other_pane() -> PaneId {
        PaneId::from_bytes([0x22; 16])
    }

    fn block(tag: u8) -> BlockId {
        BlockId::from_bytes([tag; 16])
    }

    fn ready(state: &mut ComposerState, pane: PaneId) -> u64 {
        state
            .apply(ComposerAction::EnsurePane { pane })
            .expect("ensure");
        state.snapshot(pane).expect("snap").epoch
    }

    fn running(id: BlockId, command: &str, start: u64) -> RuntimeBlockRecord {
        RuntimeBlockRecord {
            id,
            command: command.to_owned(),
            start_line: start,
            end_line: None,
            running: true,
            exit_status: None,
        }
    }

    fn completed(id: BlockId, command: &str, start: u64, end: u64) -> RuntimeBlockRecord {
        RuntimeBlockRecord {
            id,
            command: command.to_owned(),
            start_line: start,
            end_line: Some(end),
            running: false,
            exit_status: Some(0),
        }
    }

    #[test]
    fn busy_disables_submit_and_preserves_draft() {
        let pane = pane();
        let mut state = ComposerState::new();
        let epoch = ready(&mut state, pane);
        state
            .apply(ComposerAction::SetDraft {
                pane,
                text: "echo busy".into(),
                epoch,
            })
            .unwrap();
        state
            .apply(ComposerAction::SetBusy {
                pane,
                process: Some("vite".into()),
            })
            .unwrap();
        let snap = state.snapshot(pane).unwrap();
        assert_eq!(
            snap.mode,
            ComposerMode::Busy {
                process: "vite".into()
            }
        );
        assert!(!snap.can_submit);
        assert_eq!(
            state.apply(ComposerAction::Submit {
                pane,
                epoch: snap.epoch
            }),
            Err(ComposerError::SubmitDisabled)
        );
        assert_eq!(state.snapshot(pane).unwrap().draft, "echo busy");
        assert!(state.snapshot(pane).unwrap().pending_request_id.is_none());
    }

    #[test]
    fn rejected_submit_keeps_authoritative_draft() {
        let pane = pane();
        let mut state = ComposerState::new();
        let epoch = ready(&mut state, pane);
        state
            .apply(ComposerAction::SetDraft {
                pane,
                text: "git status".into(),
                epoch,
            })
            .unwrap();
        let request_id = state
            .apply(ComposerAction::Submit { pane, epoch })
            .unwrap()
            .expect("request");
        assert_eq!(state.snapshot(pane).unwrap().draft, "git status");
        assert!(!state.snapshot(pane).unwrap().can_submit);
        state
            .apply(ComposerAction::ApplyResult {
                pane,
                request_id,
                accepted: false,
            })
            .unwrap();
        let snap = state.snapshot(pane).unwrap();
        assert_eq!(snap.draft, "git status");
        assert!(snap.pending_request_id.is_none());
        assert!(snap.can_submit);
        assert_eq!(snap.mode, ComposerMode::Available);
    }

    #[test]
    fn accepted_result_clears_draft_only_for_matching_request_id() {
        let pane = pane();
        let mut state = ComposerState::new();
        let epoch = ready(&mut state, pane);
        state
            .apply(ComposerAction::SetDraft {
                pane,
                text: "printf hello".into(),
                epoch,
            })
            .unwrap();
        let request_id = state
            .apply(ComposerAction::Submit { pane, epoch })
            .unwrap()
            .expect("request");
        state
            .apply(ComposerAction::ApplyResult {
                pane,
                request_id,
                accepted: true,
            })
            .unwrap();
        let snap = state.snapshot(pane).unwrap();
        assert!(snap.draft.is_empty());
        assert!(snap.pending_request_id.is_none());
        assert!(!snap.can_submit);
    }

    #[test]
    fn stale_request_id_is_ignored_and_preserves_draft() {
        let pane = pane();
        let mut state = ComposerState::new();
        let epoch = ready(&mut state, pane);
        state
            .apply(ComposerAction::SetDraft {
                pane,
                text: "pwd".into(),
                epoch,
            })
            .unwrap();
        let request_id = state
            .apply(ComposerAction::Submit { pane, epoch })
            .unwrap()
            .expect("request");
        assert_eq!(
            state.apply(ComposerAction::ApplyResult {
                pane,
                request_id: request_id.wrapping_add(9),
                accepted: true,
            }),
            Err(ComposerError::StaleRequest)
        );
        let snap = state.snapshot(pane).unwrap();
        assert_eq!(snap.draft, "pwd");
        assert_eq!(snap.pending_request_id, Some(request_id));
        assert_eq!(
            snap.mode,
            ComposerMode::Busy {
                process: "pwd".into()
            }
        );
    }

    #[test]
    fn stale_epoch_fails_closed() {
        let pane = pane();
        let mut state = ComposerState::new();
        let epoch = ready(&mut state, pane);
        state
            .apply(ComposerAction::SetDraft {
                pane,
                text: "echo epoch".into(),
                epoch,
            })
            .unwrap();
        state.apply(ComposerAction::Submit { pane, epoch }).unwrap();
        assert_eq!(
            state.apply(ComposerAction::SetDraft {
                pane,
                text: "echo overwritten".into(),
                epoch,
            }),
            Err(ComposerError::StaleEpoch)
        );
        assert_eq!(state.snapshot(pane).unwrap().draft, "echo epoch");
        assert_eq!(
            state.apply(ComposerAction::Submit { pane, epoch }),
            Err(ComposerError::StaleEpoch)
        );
    }

    #[test]
    fn hidden_direct_terminal_route_is_not_submittable() {
        let pane = pane();
        let mut state = ComposerState::new();
        let epoch = ready(&mut state, pane);
        state
            .apply(ComposerAction::SetDraft {
                pane,
                text: "htop".into(),
                epoch,
            })
            .unwrap();
        state
            .apply(ComposerAction::ApplyPresentation {
                pane,
                mode: PresentationMode::Tui,
                input_route: InputRoute::DirectTerminal,
            })
            .unwrap();
        let snap = state.snapshot(pane).unwrap();
        assert_eq!(snap.mode, ComposerMode::Hidden);
        assert!(snap.allows_direct_terminal);
        assert!(!snap.can_submit);
        assert_eq!(snap.draft, "htop");
        assert_eq!(
            state.apply(ComposerAction::Submit {
                pane,
                epoch: snap.epoch
            }),
            Err(ComposerError::SubmitDisabled)
        );
        assert_eq!(state.snapshot(pane).unwrap().draft, "htop");
    }

    #[test]
    fn raw_direct_terminal_hides_composer_without_clearing_draft() {
        let pane = pane();
        let mut state = ComposerState::new();
        let epoch = ready(&mut state, pane);
        state
            .apply(ComposerAction::SetDraft {
                pane,
                text: "vim".into(),
                epoch,
            })
            .unwrap();
        state
            .apply(ComposerAction::ApplyPresentation {
                pane,
                mode: PresentationMode::Raw,
                input_route: InputRoute::DirectTerminal,
            })
            .unwrap();
        let snap = state.snapshot(pane).unwrap();
        assert_eq!(snap.mode, ComposerMode::Hidden);
        assert!(snap.allows_direct_terminal);
        assert_eq!(snap.draft, "vim");
    }

    #[test]
    fn runtime_projection_does_not_forge_prior_block_completion() {
        let pane = pane();
        let mut state = ComposerState::new();
        ready(&mut state, pane);
        let first = block(1);
        let second = block(2);
        state
            .apply(ComposerAction::ApplyRuntimeBlocks {
                pane,
                records: vec![running(first, "printf hello", 10)],
            })
            .unwrap();
        state
            .apply(ComposerAction::ApplyRuntimeBlocks {
                pane,
                records: vec![
                    running(first, "printf hello", 10),
                    running(second, "seq 1 1000", 20),
                ],
            })
            .unwrap();
        let snap = state.snapshot(pane).unwrap();
        assert_eq!(snap.blocks.len(), 2);
        assert_eq!(snap.blocks[0].pane, pane);
        assert_eq!(snap.blocks[0].id, first);
        assert_eq!(snap.blocks[0].state, BlockPresentationState::Running);
        assert_eq!(snap.blocks[1].id, second);
        assert_eq!(snap.blocks[1].state, BlockPresentationState::Running);
        assert_ne!(snap.blocks[0].pane, other_pane());
    }

    #[test]
    fn runtime_records_are_the_only_block_writer() {
        let pane = pane();
        let mut state = ComposerState::new();
        let epoch = ready(&mut state, pane);
        state
            .apply(ComposerAction::SetDraft {
                pane,
                text: "echo one".into(),
                epoch,
            })
            .unwrap();
        state.apply(ComposerAction::Submit { pane, epoch }).unwrap();
        assert!(state.snapshot(pane).unwrap().blocks.is_empty());
        let id = block(7);
        state
            .apply(ComposerAction::ApplyRuntimeBlocks {
                pane,
                records: vec![completed(id, "echo one", 1, 2)],
            })
            .unwrap();
        let snap = state.snapshot(pane).unwrap();
        assert_eq!(snap.blocks.len(), 1);
        assert_eq!(snap.blocks[0].id, id);
        assert_eq!(snap.blocks[0].pane, pane);
        assert_eq!(snap.blocks[0].state, BlockPresentationState::Completed);
        assert_eq!(snap.blocks[0].start_line, 1);
        assert_eq!(snap.blocks[0].end_line, Some(2));
    }

    #[test]
    fn drafts_are_isolated_per_pane() {
        let first = pane();
        let second = other_pane();
        let mut state = ComposerState::new();
        let first_epoch = ready(&mut state, first);
        let second_epoch = ready(&mut state, second);
        state
            .apply(ComposerAction::SetDraft {
                pane: first,
                text: "first draft".into(),
                epoch: first_epoch,
            })
            .unwrap();
        state
            .apply(ComposerAction::SetDraft {
                pane: second,
                text: "second draft".into(),
                epoch: second_epoch,
            })
            .unwrap();
        assert_eq!(state.snapshot(first).unwrap().draft, "first draft");
        assert_eq!(state.snapshot(second).unwrap().draft, "second draft");
        assert_eq!(
            state.apply(ComposerAction::SetDraft {
                pane: PaneId::from_bytes([0x33; 16]),
                text: "ghost".into(),
                epoch: 1,
            }),
            Err(ComposerError::UnknownPane)
        );
    }

    #[test]
    fn failed_runtime_exit_projects_failed_without_completing_siblings() {
        let pane = pane();
        let mut state = ComposerState::new();
        ready(&mut state, pane);
        let failed = block(3);
        let running_id = block(4);
        state
            .apply(ComposerAction::ApplyRuntimeBlocks {
                pane,
                records: vec![
                    RuntimeBlockRecord {
                        id: failed,
                        command: "false".into(),
                        start_line: 1,
                        end_line: Some(1),
                        running: false,
                        exit_status: Some(1),
                    },
                    running(running_id, "sleep 10", 2),
                ],
            })
            .unwrap();
        let snap = state.snapshot(pane).unwrap();
        assert_eq!(snap.blocks[0].state, BlockPresentationState::Failed);
        assert_eq!(snap.blocks[1].state, BlockPresentationState::Running);
    }
}
