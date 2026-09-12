//! One-Pane application root: the sole writable portable product-state owner.
//!
//! Composes [`ShellState`], [`PresentationSession`], and
//! [`RecoveryCoordinator`]. Runtime remains the only PTY, VT, `TerminalState`,
//! attachment/controller, and BlockTimeline authority. This module does not
//! implement composer/Block lifecycle (#881). Hosts inject clock, launch, and
//! attach attempts; this crate owns retry/deadline/stage policy.

use std::time::Duration;

use seyal_core::{AttachmentId, ExecutionId, PaneId};

use crate::presentation::{
    PresentationAction, PresentationIdentity, PresentationMode, PresentationSession,
};
use crate::recovery::{
    AttemptOutcome, LaunchResult, RecoveryCoordinator, RecoveryEffect, RecoveryStage,
};
use crate::shell::{ShellAction, ShellSnapshot, ShellState};

#[cfg(target_os = "macos")]
use crate::LocalDisplayClient;

/// Published host-contract version for versioned, size-tagged records.
pub const APP_ABI_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppError {
    UnknownPane,
    StalePane,
    StaleExecution,
    StaleAttachment,
    StaleController,
    StalePresentationEpoch,
    UnboundUnauthorized,
    AlreadyBound,
    NotController,
    DirectInputUnauthorized,
    ZeroPtyGeneration,
    Frozen,
    NoLiveClient,
    InvalidPayload,
    StaleRecoveryGeneration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationEligibility {
    Unbound,
    Flow,
    Raw,
    Tui,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeEffect {
    None,
    BoundedDetachThenTerminate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppFence {
    pub pane: PaneId,
    pub execution: Option<ExecutionId>,
    pub attachment: Option<AttachmentId>,
    pub controller: bool,
    pub presentation_epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BindingEvidence {
    pub execution: ExecutionId,
    pub attachment: AttachmentId,
    pub controller: bool,
    pub pty_generation: u64,
    pub alternate_screen: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppAction {
    Focus {
        fence: AppFence,
    },
    Bind {
        fence: AppFence,
        evidence: BindingEvidence,
    },
    Refresh {
        fence: AppFence,
    },
    SubmitInput {
        fence: AppFence,
        text: String,
    },
    Quit,
    AckEffect,
    BeginRecovery {
        now: Duration,
    },
    CompleteRecovery {
        generation: u64,
        outcome: AttemptOutcome,
        now: Duration,
        launch: Option<LaunchResult>,
    },
    FireScheduledRecovery {
        generation: u64,
        now: Duration,
    },
    AckRecoveryEffect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessibilityRole {
    Application,
    Pane,
    Composer,
    Terminal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessibilityNode {
    pub id: u64,
    pub parent: Option<u64>,
    pub role: AccessibilityRole,
    pub label: String,
    pub value: String,
    pub help: String,
    pub enabled: bool,
    pub selected: bool,
    pub focused: bool,
    pub actions: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppSnapshot {
    pub generation: u64,
    pub pane: PaneId,
    pub execution: Option<ExecutionId>,
    pub attachment: Option<AttachmentId>,
    pub controller: bool,
    pub presentation_epoch: u64,
    pub eligibility: PresentationEligibility,
    pub composer_eligible: bool,
    pub frozen: bool,
    pub last_error: Option<AppError>,
    pub pending_effect: NativeEffect,
    pub output_utf8: String,
    pub shell: ShellSnapshot,
    pub accessibility: Vec<AccessibilityNode>,
    pub recovery_stage: RecoveryStage,
    pub recovery_generation: u64,
    pub recovery_attempts: u32,
    pub recovery_effect: Option<RecoveryEffect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PaneAuthority {
    pane: PaneId,
    execution: ExecutionId,
    attachment: AttachmentId,
    controller: bool,
    pty_generation: u64,
}

/// Sole writable portable product/application state for one M001 Pane.
pub struct ApplicationRoot {
    shell: ShellState,
    presentation: PresentationSession,
    authority: Option<PaneAuthority>,
    output_utf8: String,
    snapshot_generation: u64,
    last_error: Option<AppError>,
    pending_effect: NativeEffect,
    frozen: bool,
    recovery: RecoveryCoordinator,
    pending_recovery: Vec<RecoveryEffect>,
    #[cfg(target_os = "macos")]
    client: Option<LocalDisplayClient>,
}

impl Default for ApplicationRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl ApplicationRoot {
    pub fn new() -> Self {
        let shell = ShellState::m001_local("local");
        Self {
            presentation: PresentationSession::new(None, PresentationMode::Flow),
            shell,
            authority: None,
            output_utf8: String::new(),
            snapshot_generation: 1,
            last_error: None,
            pending_effect: NativeEffect::None,
            frozen: false,
            recovery: RecoveryCoordinator::default(),
            pending_recovery: Vec::new(),
            #[cfg(target_os = "macos")]
            client: None,
        }
    }

    pub fn fence(&self) -> AppFence {
        let snap = self.shell.snapshot();
        match self.authority {
            None => AppFence {
                pane: snap.focused_pane,
                execution: None,
                attachment: None,
                controller: false,
                presentation_epoch: self.presentation.snapshot().epoch,
            },
            Some(bound) => AppFence {
                pane: bound.pane,
                execution: Some(bound.execution),
                attachment: Some(bound.attachment),
                controller: bound.controller,
                presentation_epoch: self.presentation.snapshot().epoch,
            },
        }
    }

    pub fn snapshot(&self) -> AppSnapshot {
        let shell = self.shell.snapshot();
        let eligibility = self.eligibility();
        let composer_eligible = eligibility == PresentationEligibility::Flow && !self.frozen;
        AppSnapshot {
            generation: self.snapshot_generation,
            pane: shell.focused_pane,
            execution: self.authority.map(|bound| bound.execution),
            attachment: self.authority.map(|bound| bound.attachment),
            controller: self.authority.is_some_and(|bound| bound.controller),
            presentation_epoch: self.presentation.snapshot().epoch,
            eligibility,
            composer_eligible,
            frozen: self.frozen,
            last_error: self.last_error,
            pending_effect: self.pending_effect,
            output_utf8: self.output_utf8.clone(),
            accessibility: accessibility_nodes(
                &shell,
                eligibility,
                composer_eligible,
                &self.output_utf8,
            ),
            shell,
            recovery_stage: self.recovery.state().stage,
            recovery_generation: self.recovery.state().generation,
            recovery_attempts: self.recovery.attempt_count(),
            recovery_effect: self.pending_recovery.first().copied(),
        }
    }

    pub fn apply(&mut self, action: AppAction) -> Result<(), AppError> {
        if self.frozen
            && !matches!(
                action,
                AppAction::AckEffect | AppAction::AckRecoveryEffect | AppAction::Quit
            )
        {
            return self.fail(AppError::Frozen);
        }
        let result = match action {
            AppAction::Focus { fence } => self.focus(fence),
            AppAction::Bind { fence, evidence } => self.bind(fence, evidence),
            AppAction::Refresh { fence } => self.refresh(fence),
            AppAction::SubmitInput { fence, text } => self.submit_input(fence, &text),
            AppAction::Quit => self.quit(),
            AppAction::AckEffect => self.ack_effect(),
            AppAction::BeginRecovery { now } => self.begin_recovery(now),
            AppAction::CompleteRecovery {
                generation,
                outcome,
                now,
                launch,
            } => self.complete_recovery(generation, outcome, now, launch),
            AppAction::FireScheduledRecovery { generation, now } => {
                self.fire_scheduled_recovery(generation, now)
            }
            AppAction::AckRecoveryEffect => self.ack_recovery_effect(),
        };
        match result {
            Ok(()) => {
                self.last_error = None;
                self.snapshot_generation = self.snapshot_generation.saturating_add(1);
                Ok(())
            }
            Err(error) => self.fail(error),
        }
    }

    /// Attach the existing Candidate-D client for this Pane. Does not create a
    /// PTY, VT, or Runtime registry entry.
    #[cfg(target_os = "macos")]
    pub fn attach_client(
        &mut self,
        fence: AppFence,
        client: LocalDisplayClient,
    ) -> Result<(), AppError> {
        let evidence = BindingEvidence {
            execution: client.execution_id(),
            attachment: client.attachment_id(),
            controller: matches!(
                client.role(),
                seyal_runtime::local_ipc::framing::Role::Controller
            ),
            pty_generation: client.cache().generation.max(1),
            alternate_screen: client.cache().alternate_screen,
        };
        self.apply(AppAction::Bind { fence, evidence })?;
        self.output_utf8 = project_cache_text(client.cache());
        self.client = Some(client);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    pub fn poll_client(&mut self, fence: AppFence) -> Result<(), AppError> {
        self.require_fence(fence)
            .or_else(|error| self.fail(error))?;
        let Some(client) = self.client.as_mut() else {
            return self.fail(AppError::NoLiveClient);
        };
        client.poll_prepare().map_err(|_| AppError::NoLiveClient)?;
        let alternate = client.cache().alternate_screen;
        let generation = client.cache().generation.max(1);
        self.output_utf8 = project_cache_text(client.cache());
        if let Some(bound) = self.authority.as_mut() {
            bound.pty_generation = generation;
        }
        self.derive_presentation(alternate)?;
        self.last_error = None;
        self.snapshot_generation = self.snapshot_generation.saturating_add(1);
        Ok(())
    }

    fn focus(&mut self, fence: AppFence) -> Result<(), AppError> {
        self.require_fence(fence)?;
        self.shell
            .apply(ShellAction::FocusPane { id: fence.pane })
            .map_err(|_| AppError::UnknownPane)
    }

    fn bind(&mut self, fence: AppFence, evidence: BindingEvidence) -> Result<(), AppError> {
        self.require_fence(fence)?;
        if self.authority.is_some() {
            return Err(AppError::AlreadyBound);
        }
        if evidence.pty_generation == 0 {
            return Err(AppError::ZeroPtyGeneration);
        }
        self.shell
            .apply(ShellAction::BindExecution {
                pane: fence.pane,
                execution: evidence.execution,
            })
            .map_err(|_| AppError::AlreadyBound)?;
        let identity = PresentationIdentity::new(evidence.execution, evidence.pty_generation)
            .ok_or(AppError::ZeroPtyGeneration)?;
        self.presentation
            .apply(PresentationAction::BindIdentity(identity))
            .map_err(|_| AppError::AlreadyBound)?;
        self.authority = Some(PaneAuthority {
            pane: fence.pane,
            execution: evidence.execution,
            attachment: evidence.attachment,
            controller: evidence.controller,
            pty_generation: evidence.pty_generation,
        });
        self.derive_presentation(evidence.alternate_screen)
    }

    fn refresh(&mut self, fence: AppFence) -> Result<(), AppError> {
        self.require_fence(fence)?;
        #[cfg(target_os = "macos")]
        if let Some(client) = self.client.as_ref() {
            self.output_utf8 = project_cache_text(client.cache());
            return self.derive_presentation(client.cache().alternate_screen);
        }
        self.derive_presentation(self.eligibility() == PresentationEligibility::Tui)
    }

    fn submit_input(&mut self, fence: AppFence, text: &str) -> Result<(), AppError> {
        self.require_fence(fence)?;
        if self.authority.is_none() {
            return Err(AppError::UnboundUnauthorized);
        }
        if !self.authority.is_some_and(|bound| bound.controller) {
            return Err(AppError::NotController);
        }
        match self.eligibility() {
            PresentationEligibility::Raw | PresentationEligibility::Tui => {}
            PresentationEligibility::Unbound => return Err(AppError::UnboundUnauthorized),
            PresentationEligibility::Flow => return Err(AppError::DirectInputUnauthorized),
        }
        if text.is_empty() {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        {
            let Some(client) = self.client.as_mut() else {
                return Err(AppError::NoLiveClient);
            };
            client
                .submit_committed_text(text)
                .map_err(|_| AppError::InvalidPayload)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = text;
            Err(AppError::NoLiveClient)
        }
    }

    fn quit(&mut self) -> Result<(), AppError> {
        self.frozen = true;
        self.pending_effect = NativeEffect::BoundedDetachThenTerminate;
        Ok(())
    }

    fn ack_effect(&mut self) -> Result<(), AppError> {
        self.pending_effect = NativeEffect::None;
        Ok(())
    }

    fn begin_recovery(&mut self, now: Duration) -> Result<(), AppError> {
        self.pending_recovery = self.recovery.begin_episode(now);
        Ok(())
    }

    fn complete_recovery(
        &mut self,
        generation: u64,
        outcome: AttemptOutcome,
        now: Duration,
        launch: Option<LaunchResult>,
    ) -> Result<(), AppError> {
        let stale = generation != self.recovery.state().generation;
        self.pending_recovery = self
            .recovery
            .complete_attempt(generation, outcome, now, launch);
        if stale {
            return Err(AppError::StaleRecoveryGeneration);
        }
        Ok(())
    }

    fn fire_scheduled_recovery(&mut self, generation: u64, now: Duration) -> Result<(), AppError> {
        if generation != self.recovery.state().generation {
            return self.fail(AppError::StaleRecoveryGeneration);
        }
        self.pending_recovery = self.recovery.scheduled_fire(generation, now);
        Ok(())
    }

    fn ack_recovery_effect(&mut self) -> Result<(), AppError> {
        if !self.pending_recovery.is_empty() {
            self.pending_recovery.remove(0);
        }
        Ok(())
    }

    fn require_fence(&self, fence: AppFence) -> Result<(), AppError> {
        let current = self.fence();
        if self.shell.pane_execution(fence.pane).is_err() {
            return Err(AppError::UnknownPane);
        }
        if fence.pane != current.pane {
            return Err(AppError::StalePane);
        }
        if fence.execution != current.execution {
            return Err(AppError::StaleExecution);
        }
        if fence.attachment != current.attachment {
            return Err(AppError::StaleAttachment);
        }
        if fence.controller != current.controller {
            return Err(AppError::StaleController);
        }
        if fence.presentation_epoch != current.presentation_epoch {
            return Err(AppError::StalePresentationEpoch);
        }
        Ok(())
    }

    fn derive_presentation(&mut self, alternate_screen: bool) -> Result<(), AppError> {
        let Some(bound) = self.authority else {
            return Ok(());
        };
        let desired = if alternate_screen {
            PresentationMode::Tui
        } else {
            PresentationMode::Flow
        };
        let current = self.presentation.snapshot();
        if current.mode == desired {
            return Ok(());
        }
        let identity = PresentationIdentity::new(bound.execution, bound.pty_generation)
            .ok_or(AppError::ZeroPtyGeneration)?;
        self.presentation
            .apply(PresentationAction::Transition {
                mode: desired,
                identity,
                explicit: false,
                epoch: current.epoch,
            })
            .map_err(|_| AppError::StalePresentationEpoch)
    }

    fn eligibility(&self) -> PresentationEligibility {
        if self.authority.is_none() {
            return PresentationEligibility::Unbound;
        }
        match self.presentation.snapshot().mode {
            PresentationMode::Flow => PresentationEligibility::Flow,
            PresentationMode::Raw => PresentationEligibility::Raw,
            PresentationMode::Tui => PresentationEligibility::Tui,
        }
    }

    fn fail(&mut self, error: AppError) -> Result<(), AppError> {
        self.last_error = Some(error);
        Err(error)
    }
}

fn accessibility_nodes(
    shell: &ShellSnapshot,
    eligibility: PresentationEligibility,
    composer_eligible: bool,
    output: &str,
) -> Vec<AccessibilityNode> {
    let pane_title = shell
        .panes
        .iter()
        .find(|pane| pane.id == shell.focused_pane)
        .map(|pane| pane.title.clone())
        .unwrap_or_else(|| "Pane".to_owned());
    let mut nodes = vec![
        AccessibilityNode {
            id: 1,
            parent: None,
            role: AccessibilityRole::Application,
            label: "Seyal".to_owned(),
            value: String::new(),
            help: "Seyal application root".to_owned(),
            enabled: true,
            selected: false,
            focused: false,
            actions: 0,
        },
        AccessibilityNode {
            id: 2,
            parent: Some(1),
            role: AccessibilityRole::Pane,
            label: pane_title,
            value: String::new(),
            help: "Terminal pane".to_owned(),
            enabled: true,
            selected: true,
            focused: true,
            actions: 1,
        },
    ];
    match eligibility {
        PresentationEligibility::Unbound => {}
        PresentationEligibility::Flow if composer_eligible => nodes.push(AccessibilityNode {
            id: 3,
            parent: Some(2),
            role: AccessibilityRole::Composer,
            label: "Composer".to_owned(),
            value: String::new(),
            help: "Flow composer is eligible; draft lifecycle is #881".to_owned(),
            enabled: true,
            selected: false,
            focused: false,
            actions: 0,
        }),
        PresentationEligibility::Flow => {}
        PresentationEligibility::Raw | PresentationEligibility::Tui => {
            nodes.push(AccessibilityNode {
                id: 3,
                parent: Some(2),
                role: AccessibilityRole::Terminal,
                label: "Terminal".to_owned(),
                value: output.to_owned(),
                help: "Direct terminal presentation".to_owned(),
                enabled: true,
                selected: false,
                focused: true,
                actions: 0,
            })
        }
    }
    nodes
}

#[cfg(target_os = "macos")]
fn project_cache_text(cache: &seyal_runtime::display::DisplayCache) -> String {
    use seyal_runtime::display::DisplayCellRole;
    let mut text = String::new();
    for (index, cell) in cache.cells.iter().enumerate() {
        if cell.role == DisplayCellRole::Lead {
            if !cell.text.is_empty() {
                text.push_str(&String::from_utf8_lossy(&cell.text));
            } else if cell.scalar != ' ' && cell.scalar != '\0' {
                text.push(cell.scalar);
            }
        }
        if cache.columns > 0 && (index + 1) % usize::from(cache.columns) == 0 {
            text.push('\n');
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::InputRoute;

    fn evidence(tag: u8, controller: bool, alternate: bool) -> BindingEvidence {
        BindingEvidence {
            execution: ExecutionId::from_bytes([tag; 16]),
            attachment: AttachmentId::from_bytes([tag.wrapping_add(1); 16]),
            controller,
            pty_generation: 1,
            alternate_screen: alternate,
        }
    }

    #[test]
    fn new_root_is_one_unbound_pane() {
        let root = ApplicationRoot::new();
        let snap = root.snapshot();
        assert_eq!(snap.shell.panes.len(), 1);
        assert!(snap.execution.is_none());
        assert_eq!(snap.eligibility, PresentationEligibility::Unbound);
        assert!(!snap.composer_eligible);
        assert_eq!(snap.generation, 1);
        assert_eq!(root.snapshot(), root.snapshot());
    }

    #[test]
    fn unbound_cannot_authorize_flow_or_composer() {
        let mut root = ApplicationRoot::new();
        let fence = root.fence();
        assert_eq!(
            root.apply(AppAction::SubmitInput {
                fence,
                text: "echo".into(),
            }),
            Err(AppError::UnboundUnauthorized)
        );
        let snap = root.snapshot();
        assert_eq!(snap.eligibility, PresentationEligibility::Unbound);
        assert!(!snap.composer_eligible);
        assert!(!snap
            .accessibility
            .iter()
            .any(|node| node.role == AccessibilityRole::Composer));
        assert_eq!(
            root.presentation.snapshot().input_route,
            InputRoute::Composer
        );
        assert_ne!(snap.eligibility, PresentationEligibility::Flow);
    }

    #[test]
    fn bind_then_focus_keeps_one_execution() {
        let mut root = ApplicationRoot::new();
        let fence = root.fence();
        let bound = evidence(1, true, false);
        root.apply(AppAction::Bind {
            fence,
            evidence: bound,
        })
        .unwrap();
        let snap = root.snapshot();
        assert_eq!(snap.execution, Some(bound.execution));
        assert_eq!(snap.eligibility, PresentationEligibility::Flow);
        assert!(snap.composer_eligible);
        let focused = snap.pane;
        root.apply(AppAction::Focus {
            fence: root.fence(),
        })
        .unwrap();
        assert_eq!(root.snapshot().pane, focused);
        assert_eq!(root.snapshot().execution, Some(bound.execution));
    }

    #[test]
    fn alternate_screen_evidence_derives_tui_not_host_policy() {
        let mut root = ApplicationRoot::new();
        let fence = root.fence();
        root.apply(AppAction::Bind {
            fence,
            evidence: evidence(2, true, true),
        })
        .unwrap();
        let snap = root.snapshot();
        assert_eq!(snap.eligibility, PresentationEligibility::Tui);
        assert!(!snap.composer_eligible);
        assert!(snap
            .accessibility
            .iter()
            .any(|node| node.role == AccessibilityRole::Terminal));
    }

    #[test]
    fn stale_identities_fail_closed_and_are_not_retried() {
        let mut root = ApplicationRoot::new();
        let unbound = root.fence();
        root.apply(AppAction::Bind {
            fence: unbound,
            evidence: evidence(3, true, false),
        })
        .unwrap();
        let generation = root.snapshot().generation;
        assert_eq!(
            root.apply(AppAction::Focus { fence: unbound }),
            Err(AppError::StaleExecution)
        );
        assert_eq!(root.snapshot().generation, generation);
        assert_eq!(root.snapshot().last_error, Some(AppError::StaleExecution));

        let current = root.fence();
        let mut stale_pane = current;
        stale_pane.pane = PaneId::from_bytes([0xff; 16]);
        assert_eq!(
            root.apply(AppAction::Focus { fence: stale_pane }),
            Err(AppError::UnknownPane)
        );

        let mut stale_attach = current;
        stale_attach.attachment = Some(AttachmentId::from_bytes([0xab; 16]));
        assert_eq!(
            root.apply(AppAction::Refresh {
                fence: stale_attach
            }),
            Err(AppError::StaleAttachment)
        );

        let mut stale_controller = current;
        stale_controller.controller = false;
        assert_eq!(
            root.apply(AppAction::Refresh {
                fence: stale_controller
            }),
            Err(AppError::StaleController)
        );

        let mut stale_epoch = current;
        stale_epoch.presentation_epoch = current.presentation_epoch.wrapping_add(9);
        assert_eq!(
            root.apply(AppAction::Refresh { fence: stale_epoch }),
            Err(AppError::StalePresentationEpoch)
        );
        assert_eq!(
            root.snapshot().execution,
            Some(evidence(3, true, false).execution)
        );
    }

    #[test]
    fn observer_and_flow_cannot_submit_direct_input() {
        let mut root = ApplicationRoot::new();
        root.apply(AppAction::Bind {
            fence: root.fence(),
            evidence: evidence(4, false, false),
        })
        .unwrap();
        assert_eq!(
            root.apply(AppAction::SubmitInput {
                fence: root.fence(),
                text: "x".into(),
            }),
            Err(AppError::NotController)
        );

        let mut controller = ApplicationRoot::new();
        controller
            .apply(AppAction::Bind {
                fence: controller.fence(),
                evidence: evidence(5, true, false),
            })
            .unwrap();
        assert_eq!(
            controller.apply(AppAction::SubmitInput {
                fence: controller.fence(),
                text: "x".into(),
            }),
            Err(AppError::DirectInputUnauthorized)
        );
    }

    #[test]
    fn tui_controller_without_client_is_authorized_but_has_no_second_pty() {
        let mut root = ApplicationRoot::new();
        root.apply(AppAction::Bind {
            fence: root.fence(),
            evidence: evidence(6, true, true),
        })
        .unwrap();
        assert_eq!(
            root.apply(AppAction::SubmitInput {
                fence: root.fence(),
                text: "x".into(),
            }),
            Err(AppError::NoLiveClient)
        );
        assert_eq!(root.snapshot().shell.panes.len(), 1);
        assert_eq!(
            root.snapshot().execution.unwrap(),
            evidence(6, true, true).execution
        );
    }

    #[test]
    fn quit_freezes_and_emits_one_native_effect() {
        let mut root = ApplicationRoot::new();
        root.apply(AppAction::Quit).unwrap();
        let snap = root.snapshot();
        assert!(snap.frozen);
        assert_eq!(
            snap.pending_effect,
            NativeEffect::BoundedDetachThenTerminate
        );
        assert_eq!(
            root.apply(AppAction::Focus {
                fence: root.fence()
            }),
            Err(AppError::Frozen)
        );
        root.apply(AppAction::AckEffect).unwrap();
        assert_eq!(root.snapshot().pending_effect, NativeEffect::None);
    }

    #[test]
    fn unknown_pane_does_not_route_across_identities() {
        let mut root = ApplicationRoot::new();
        let mut fence = root.fence();
        fence.pane = PaneId::new();
        assert_eq!(
            root.apply(AppAction::Focus { fence }),
            Err(AppError::UnknownPane)
        );
    }

    #[test]
    fn recovery_retry_ladder_and_stale_generation_fail_closed() {
        let mut root = ApplicationRoot::new();
        root.apply(AppAction::BeginRecovery {
            now: Duration::ZERO,
        })
        .unwrap();
        let first = root.snapshot();
        assert_eq!(first.recovery_stage, RecoveryStage::Discovering);
        assert_eq!(first.recovery_attempts, 1);
        assert_eq!(
            first.recovery_effect,
            Some(RecoveryEffect::PerformAttempt {
                generation: first.recovery_generation,
                remaining: Duration::from_secs(1),
            })
        );

        root.apply(AppAction::CompleteRecovery {
            generation: first.recovery_generation,
            outcome: AttemptOutcome::ControllerBusy,
            now: Duration::ZERO,
            launch: None,
        })
        .unwrap();
        let scheduled = root.snapshot();
        assert_eq!(
            scheduled.recovery_stage,
            RecoveryStage::WaitingForController
        );
        assert_eq!(
            scheduled.recovery_effect,
            Some(RecoveryEffect::Schedule {
                generation: first.recovery_generation,
                delay: Duration::from_millis(10),
            })
        );

        root.apply(AppAction::BeginRecovery {
            now: Duration::from_millis(5),
        })
        .unwrap();
        let second = root.snapshot();
        assert_ne!(second.recovery_generation, first.recovery_generation);
        assert_eq!(
            root.apply(AppAction::FireScheduledRecovery {
                generation: first.recovery_generation,
                now: Duration::from_millis(15),
            }),
            Err(AppError::StaleRecoveryGeneration)
        );
        assert_eq!(
            root.apply(AppAction::CompleteRecovery {
                generation: first.recovery_generation,
                outcome: AttemptOutcome::Opened {
                    handle: 9,
                    adopted: true,
                },
                now: Duration::from_millis(15),
                launch: None,
            }),
            Err(AppError::StaleRecoveryGeneration)
        );
        assert_eq!(
            root.snapshot().recovery_effect,
            Some(RecoveryEffect::DisposeHandle(9))
        );
        assert_eq!(root.snapshot().recovery_stage, RecoveryStage::Discovering);
        assert_eq!(
            root.snapshot().recovery_generation,
            second.recovery_generation
        );
    }

    #[test]
    fn recovery_endpoint_missing_launches_once_then_seven_attempts() {
        use crate::recovery::{EPISODE_DEADLINE, MAXIMUM_ATTEMPTS, RETRY_DELAYS};

        let mut root = ApplicationRoot::new();
        root.apply(AppAction::BeginRecovery {
            now: Duration::ZERO,
        })
        .unwrap();
        let generation = root.snapshot().recovery_generation;
        let mut now = Duration::ZERO;
        let mut launches = 0u32;
        for _ in 0..MAXIMUM_ATTEMPTS {
            root.apply(AppAction::AckRecoveryEffect).unwrap();
            root.apply(AppAction::CompleteRecovery {
                generation,
                outcome: AttemptOutcome::EndpointMissing,
                now,
                launch: Some(LaunchResult::Started),
            })
            .unwrap();
            if matches!(
                root.snapshot().recovery_effect,
                Some(RecoveryEffect::LaunchHelper { .. })
            ) {
                launches += 1;
                root.apply(AppAction::AckRecoveryEffect).unwrap();
            }
            if let Some(RecoveryEffect::Schedule { delay, .. }) = root.snapshot().recovery_effect {
                now += delay;
                if now >= EPISODE_DEADLINE {
                    break;
                }
                root.apply(AppAction::AckRecoveryEffect).unwrap();
                root.apply(AppAction::FireScheduledRecovery { generation, now })
                    .unwrap();
            }
        }
        assert_eq!(launches, 1);
        assert_eq!(root.snapshot().recovery_attempts, MAXIMUM_ATTEMPTS);
        assert_eq!(root.snapshot().recovery_stage, RecoveryStage::Exhausted);
        assert_eq!(RETRY_DELAYS.len() as u32 + 1, MAXIMUM_ATTEMPTS);
    }
}
