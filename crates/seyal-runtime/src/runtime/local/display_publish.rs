use std::collections::HashMap;

use crate::{
    ExecutionId, RuntimeError,
    display::{self, EncodedDisplayBatch},
    local_ipc::{
        connection::ConnectionState as LocalIpcConnState,
        connection::DeltaEnqueueResult,
        framing::{self, ErrorCode, MessageType},
    },
};

#[cfg(feature = "test-fault-injection")]
use crate::test_fault::{self, FaultPoint};
use seyal_protocol::pass8::{CAP_BLOCK_METADATA, encode_block_state_frame};

use super::super::Runtime;
use super::super::lifecycle::BlockCompletion;

#[derive(Clone, Copy)]
pub(super) struct PublishedDisplay {
    pub(super) generation: u64,
    pub(super) rows: u16,
    pub(super) columns: u16,
}

impl Runtime {
    pub(in crate::runtime) fn notify_local_ipc_execution_finalized(
        &mut self,
        execution_id: ExecutionId,
        block_completion: BlockCompletion,
    ) {
        let notifications = {
            let Some(state) = self.local_ipc.as_mut() else {
                return;
            };
            let pairs = state
                .attachments
                .attachments_with_connections_for_execution(execution_id);
            let notifications = pairs
                .iter()
                .map(|(_, token)| {
                    let block_capable = state
                        .connections
                        .get(token)
                        .is_some_and(|meta| meta.client_capabilities & CAP_BLOCK_METADATA != 0);
                    (*token, block_capable)
                })
                .collect::<Vec<_>>();
            state.attachments.remove_all_for_execution(execution_id);
            state.published.remove(&execution_id);
            for (_, token) in &pairs {
                state.pending_resync_set.remove(token);
                if let Some(meta) = state.connections.get_mut(token) {
                    meta.attachment = None;
                }
            }
            notifications
        };

        let completion_frame = match block_completion {
            BlockCompletion::Completed(record) => {
                #[cfg(feature = "test-fault-injection")]
                if test_fault::take(FaultPoint::BlockCompletionEncode) {
                    Err(())
                } else {
                    encode_block_state_frame(&record.to_wire())
                        .map(Some)
                        .map_err(|_| ())
                }
                #[cfg(not(feature = "test-fault-injection"))]
                {
                    encode_block_state_frame(&record.to_wire())
                        .map(Some)
                        .map_err(|_| ())
                }
            }
            BlockCompletion::Failed => Err(()),
            BlockCompletion::None => Ok(None),
        };

        for (token, block_capable) in notifications {
            if block_capable {
                match &completion_frame {
                    Ok(Some(frame)) => {
                        #[cfg(feature = "test-fault-injection")]
                        if test_fault::take(FaultPoint::BlockCompletionAdmission) {
                            self.close_local_connection(token);
                            continue;
                        }
                        if !self.send_after_display_frame(token, frame.clone()) {
                            continue;
                        }
                    }
                    Err(()) => {
                        self.close_local_connection(token);
                        continue;
                    }
                    Ok(None) => {}
                }
            }

            let message = framing::LifecycleMessage {
                execution_id,
                lifecycle: framing::Lifecycle::Finalized,
            };
            if self.send_after_display_frame(
                token,
                framing::encode_frame(MessageType::Lifecycle, &message.encode()),
            ) && let Some(state) = self.local_ipc.as_mut()
            {
                state.server.set_state(token, LocalIpcConnState::Ready);
            }
        }
    }

    /// Admit one authoritative final display snapshot for every attached client.
    ///
    /// Finalization cannot rely on asynchronous resync recovery: that queue is
    /// deliberately budgeted per poll and is retired with the execution. This
    /// bounded snapshot admission makes the established final-display ordering
    /// explicit even when no new projection update exists in the final turn.
    /// It never waits for a client read or acknowledgement; the existing
    /// replaceable display slot and after-display queue preserve ordering.
    pub(in crate::runtime) fn publish_final_display_snapshot(&mut self, execution_id: ExecutionId) {
        let viewers = self.local_ipc.as_ref().map_or_else(Vec::new, |state| {
            state
                .attachments
                .attachments_with_connections_for_execution(execution_id)
        });
        if viewers.is_empty() {
            return;
        }

        let batch = self
            .entries
            .get(&execution_id)
            .map(|entry| entry.execution.projection_snapshot())
            .and_then(|snapshot| display::encode_snapshot(&snapshot).ok());
        match batch {
            Some(batch) => {
                for (_, token) in viewers {
                    let _ = self.send_snapshot_batch(token, batch.clone());
                }
            }
            None => {
                // A client must never receive Finalized behind stale display.
                // If final display cannot be produced, fail that connection
                // closed while Runtime execution cleanup continues normally.
                for (_, token) in viewers {
                    self.close_local_connection(token);
                }
            }
        }
    }

    pub(in crate::runtime) fn publish_display_updates(&mut self) {
        self.service_pending_resyncs();

        let execution_ids = self.local_ipc.as_ref().map_or_else(Vec::new, |state| {
            self.entries
                .keys()
                .copied()
                .filter(|id| state.attachments.attachments_for_execution(*id) > 0)
                .collect()
        });

        for execution_id in execution_ids {
            let update = self
                .entries
                .get_mut(&execution_id)
                .and_then(|entry| entry.execution.take_projection_update());
            let Some(update) = update else {
                continue;
            };
            let previous = self
                .local_ipc
                .as_ref()
                .and_then(|state| state.published.get(&execution_id).copied());
            if previous.is_some_and(|value| update.source_damage_generation <= value.generation) {
                continue;
            }
            let viewers = self.local_ipc.as_ref().map_or_else(Vec::new, |state| {
                state
                    .attachments
                    .attachments_with_connections_for_execution(execution_id)
            });
            if viewers.is_empty() {
                continue;
            }

            // `published` tracks the last generation successfully encoded for
            // fanout. Advancing it after DisplayUnavailable would make later
            // deltas use a base no viewer received (multi-viewer split-brain).
            let use_snapshot = match previous {
                None => true,
                Some(value) => value.rows != update.rows || value.columns != update.columns,
            };
            let encode_ok = if use_snapshot {
                match self.encode_projection_snapshot(execution_id) {
                    Some(batch) => {
                        for (_, token) in &viewers {
                            let _ = self.send_snapshot_batch(*token, batch.clone());
                        }
                        true
                    }
                    None => {
                        for (_, token) in viewers {
                            self.send_error(
                                token,
                                ErrorCode::DisplayUnavailable,
                                MessageType::DisplaySnapshot as u16,
                            );
                            self.schedule_snapshot_recovery(token);
                        }
                        false
                    }
                }
            } else if let Some(previous) = previous {
                let base_generation = previous.generation;
                match self.encode_projection_delta(&update, base_generation) {
                    Ok(delta) => {
                        for (_, token) in viewers {
                            let result = self.local_ipc.as_mut().and_then(|state| {
                                state.server.try_enqueue_delta(token, delta.clone()).ok()
                            });
                            match result {
                                Some(DeltaEnqueueResult::Queued | DeltaEnqueueResult::Skipped) => {
                                    self.sync_local_writable(token);
                                }
                                Some(DeltaEnqueueResult::NeedSnapshot) => {
                                    self.schedule_snapshot_recovery(token);
                                }
                                None => self.close_local_connection(token),
                            }
                        }
                        true
                    }
                    Err(_) => {
                        for (_, token) in viewers {
                            self.send_error(
                                token,
                                ErrorCode::DisplayUnavailable,
                                MessageType::DisplayDelta as u16,
                            );
                            self.schedule_snapshot_recovery(token);
                        }
                        false
                    }
                }
            } else {
                false
            };

            if let Some(state) = self.local_ipc.as_mut() {
                if encode_ok {
                    state.published.insert(
                        execution_id,
                        PublishedDisplay {
                            generation: update.source_damage_generation,
                            rows: update.rows,
                            columns: update.columns,
                        },
                    );
                } else {
                    // Drop stale bookkeeping so the next successful fanout or
                    // resync snapshot re-establishes an authoritative base.
                    state.published.remove(&execution_id);
                }
            }
        }
    }

    pub(super) fn encode_projection_snapshot(
        &self,
        execution_id: ExecutionId,
    ) -> Option<EncodedDisplayBatch> {
        #[cfg(feature = "test-fault-injection")]
        if test_fault::take(FaultPoint::DisplayEncode) {
            return None;
        }
        self.entries
            .get(&execution_id)
            .map(|entry| entry.execution.projection_snapshot())
            .and_then(|snapshot| display::encode_snapshot(&snapshot).ok())
    }

    pub(super) fn encode_projection_delta(
        &self,
        update: &seyal_exec::TerminalProjectionUpdate,
        base_generation: u64,
    ) -> Result<EncodedDisplayBatch, display::DisplayError> {
        #[cfg(feature = "test-fault-injection")]
        if test_fault::take(FaultPoint::DisplayEncode) {
            return Err(display::DisplayError::InvalidDamage);
        }
        display::encode_delta(update, base_generation)
    }
}
