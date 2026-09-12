use seyal_client::app::{
    AppAction, AppError, AppFence, ApplicationRoot, BindingEvidence, PresentationEligibility,
};
use seyal_core::{AttachmentId, ExecutionId, PaneId};

fn evidence(tag: u8, alternate: bool) -> BindingEvidence {
    BindingEvidence {
        execution: ExecutionId::from_bytes([tag; 16]),
        attachment: AttachmentId::from_bytes([tag.wrapping_add(3); 16]),
        controller: true,
        pty_generation: 1,
        alternate_screen: alternate,
    }
}

#[derive(Clone, Copy)]
enum Step {
    Focus,
    Bind,
    Refresh,
    Submit,
    Quit,
    StalePane,
    StaleExecution,
}

fn apply_step(root: &mut ApplicationRoot, step: Step, seed: u8) -> Result<(), AppError> {
    let fence = root.fence();
    match step {
        Step::Focus => root.apply(AppAction::Focus { fence }),
        Step::Bind => root.apply(AppAction::Bind {
            fence,
            evidence: evidence(seed, seed.is_multiple_of(2)),
        }),
        Step::Refresh => root.apply(AppAction::Refresh { fence }),
        Step::Submit => root.apply(AppAction::SubmitInput {
            fence,
            text: "x".into(),
        }),
        Step::Quit => root.apply(AppAction::Quit),
        Step::StalePane => {
            let mut stale = fence;
            stale.pane = PaneId::from_bytes([0xee; 16]);
            root.apply(AppAction::Focus { fence: stale })
        }
        Step::StaleExecution => {
            let mut stale = fence;
            stale.execution = Some(ExecutionId::from_bytes([0xdd; 16]));
            root.apply(AppAction::Refresh { fence: stale })
        }
    }
}

#[test]
fn random_sequences_never_create_a_second_authority() {
    let steps = [
        Step::Focus,
        Step::Bind,
        Step::Refresh,
        Step::Submit,
        Step::Quit,
        Step::StalePane,
        Step::StaleExecution,
    ];
    for seed in 1u8..=64 {
        let mut root = ApplicationRoot::new();
        let mut rng = u64::from(seed).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        for _ in 0..12 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let step = steps[(rng as usize) % steps.len()];
            let before = root.snapshot();
            let _ = apply_step(&mut root, step, seed);
            let after = root.snapshot();
            assert_eq!(after.shell.panes.len(), 1);
            if let Some(execution) = after.execution {
                assert_eq!(after.shell.panes[0].execution, Some(execution));
                assert_eq!(
                    after
                        .shell
                        .panes
                        .iter()
                        .filter(|pane| pane.execution.is_some())
                        .count(),
                    1
                );
            }
            if before.execution.is_some() && after.execution.is_some() {
                assert_eq!(before.execution, after.execution);
            }
            if after.eligibility == PresentationEligibility::Unbound {
                assert!(!after.composer_eligible);
            }
        }
    }
}

#[test]
fn failed_stale_sequence_does_not_advance_generation() {
    let mut root = ApplicationRoot::new();
    root.apply(AppAction::Bind {
        fence: root.fence(),
        evidence: evidence(9, false),
    })
    .unwrap();
    let generation = root.snapshot().generation;
    let fence = AppFence {
        pane: root.fence().pane,
        execution: None,
        attachment: None,
        controller: false,
        presentation_epoch: root.fence().presentation_epoch,
    };
    assert!(root.apply(AppAction::Focus { fence }).is_err());
    assert_eq!(root.snapshot().generation, generation);
}
