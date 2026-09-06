#[cfg(target_os = "macos")]
use seyal_exec::{CommandSpec, ShellIntegrationEvent, ShellIntegrationToken};

#[cfg(target_os = "macos")]
use crate::command_block_timeline::{CommandBlockId, MAX_COMMAND_BYTES};
use crate::{ExecutionId, RuntimeError};

#[cfg(target_os = "macos")]
use super::entry::PendingComposerCommand;
use super::Runtime;

#[cfg(target_os = "macos")]
fn issue_shell_integration_token() -> Result<ShellIntegrationToken, RuntimeError> {
    let mut token = [0u8; 16];
    let mut source = std::fs::File::open("/dev/urandom")?;
    use std::io::Read;
    source.read_exact(&mut token)?;
    Ok(ShellIntegrationToken::from_bytes(token))
}

#[cfg(all(test, target_os = "macos"))]
mod composer_wrapper_tests {
    use super::*;

    #[test]
    fn zsh_hook_command_binds_markers_to_nonce_without_eval_wrapper() {
        let token = ShellIntegrationToken::from_bytes([0xabu8; 16]);
        let wrapped = zsh_composer_command("printf 'ok'; false", token);
        assert!(wrapped.contains("__seyal_block__ abababababababababababababababab"));
        assert!(wrapped.contains("133;C;%s"));
        assert!(wrapped.contains("133;D;%s;%s"));
        assert!(!wrapped.contains("eval "));
        // $? must be captured before any conditional in precmd.
        let precmd = wrapped
            .split("_seyal_block_precmd() {")
            .nth(1)
            .expect("precmd present");
        let status_pos = precmd
            .find("local _seyal_status=$?")
            .expect("status capture");
        let gate_pos = precmd
            .find("[[ -n \"$_seyal_active_token\" ]]")
            .expect("token gate");
        assert!(status_pos < gate_pos);
        // Marker function itself emits C so first-command install works.
        assert!(wrapped.contains("__seyal_block__() { _seyal_active_token=$1;"));
    }

    #[test]
    fn only_zsh_is_block_capable_and_other_shells_remain_raw() {
        assert_eq!(
            shell_integration_mode(&CommandSpec::new("/bin/zsh")),
            ShellIntegrationMode::ZshHook
        );
        assert_eq!(
            shell_integration_mode(&CommandSpec::new("/bin/sh")),
            ShellIntegrationMode::Unsupported
        );
    }

    #[test]
    fn busy_composer_admission_is_a_correlated_result_not_a_transport_error() {
        assert_eq!(ComposerAdmission::Busy, ComposerAdmission::Busy);
        assert_ne!(ComposerAdmission::Busy, ComposerAdmission::Unsupported);
    }

    #[test]
    fn composer_wrap_is_marker_first_after_optional_install() {
        let token = ShellIntegrationToken::from_bytes([0x11u8; 16]);
        let wrapped = zsh_composer_command("false", token);
        let marker = "__seyal_block__ 11111111111111111111111111111111";
        let marker_pos = wrapped.find(marker).expect("marker present");
        let command_pos = wrapped.rfind("; false").expect("user command present");
        assert!(marker_pos < command_pos);
        // Install bootstrap must not prefix the observed marker line after install.
        assert!(wrapped.ends_with(&format!("{marker}; false")));
    }

    #[test]
    fn live_zsh_pty_emits_trusted_c_and_real_exit_status_for_first_and_second_commands() {
        use std::time::{Duration, Instant};

        use seyal_exec::{
            CommandSpec, ReadOutcome, ShellIntegrationEvent, TerminalExecution, WindowSize,
        };

        let size = WindowSize::cells(80, 24).expect("size");
        let mut execution =
            TerminalExecution::spawn(&CommandSpec::new("/bin/zsh").args(["-i", "-f"]), size)
                .expect("spawn interactive zsh");

        let first_token = ShellIntegrationToken::from_bytes([0xABu8; 16]);
        let first = format!("{}\r", zsh_composer_command("false", first_token));
        execution
            .write_input_bounded(first.as_bytes(), Duration::from_secs(2))
            .expect("write first wrap");

        let mut saw_start = false;
        let mut finished_status = None;
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut buffer = [0u8; 4096];
        while Instant::now() < deadline && finished_status.is_none() {
            match execution.read_output(&mut buffer).expect("read") {
                ReadOutcome::Bytes(_) => {
                    while let Some(event) = execution.take_shell_integration_event() {
                        match event {
                            ShellIntegrationEvent::CommandStarted { token } => {
                                assert_eq!(token, first_token);
                                saw_start = true;
                            }
                            ShellIntegrationEvent::CommandFinished { token, exit_status } => {
                                assert_eq!(token, first_token);
                                finished_status = Some(exit_status);
                            }
                        }
                    }
                }
                ReadOutcome::WouldBlock => {
                    let _ = execution.wait_readable(Duration::from_millis(50));
                }
                ReadOutcome::Eof => break,
            }
        }
        assert!(saw_start, "first composer wrap must emit trusted C");
        assert_eq!(finished_status, Some(1), "false must report exit status 1");

        let second_token = ShellIntegrationToken::from_bytes([0xCDu8; 16]);
        let second = format!("{}\r", zsh_composer_command("true", second_token));
        execution
            .write_input_bounded(second.as_bytes(), Duration::from_secs(2))
            .expect("write second wrap");

        saw_start = false;
        finished_status = None;
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline && finished_status.is_none() {
            match execution.read_output(&mut buffer).expect("read") {
                ReadOutcome::Bytes(_) => {
                    while let Some(event) = execution.take_shell_integration_event() {
                        match event {
                            ShellIntegrationEvent::CommandStarted { token } => {
                                assert_eq!(token, second_token);
                                saw_start = true;
                            }
                            ShellIntegrationEvent::CommandFinished { token, exit_status } => {
                                assert_eq!(token, second_token);
                                finished_status = Some(exit_status);
                            }
                        }
                    }
                }
                ReadOutcome::WouldBlock => {
                    let _ = execution.wait_readable(Duration::from_millis(50));
                }
                ReadOutcome::Eof => break,
            }
        }
        assert!(saw_start, "second composer wrap must emit trusted C");
        assert_eq!(finished_status, Some(0), "true must report exit status 0");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(target_os = "macos")]
pub(super) enum ShellIntegrationMode {
    ZshHook,
    Unsupported,
}

#[cfg(target_os = "macos")]
pub(super) fn shell_integration_mode(command: &CommandSpec) -> ShellIntegrationMode {
    match command.program().to_string_lossy().as_ref() {
        "/bin/zsh" | "zsh" => ShellIntegrationMode::ZshHook,
        _ => ShellIntegrationMode::Unsupported,
    }
}

#[cfg(target_os = "macos")]
fn zsh_composer_command(command: &str, token: ShellIntegrationToken) -> String {
    let mut token_hex = String::with_capacity(32);
    token.write_hex(&mut token_hex);
    // Install hooks once. Emit OSC-133 C from `__seyal_block__` itself so the
    // first wrapped command does not depend on preexec seeing an already-
    // installed hook, and so later lines are marker-first (`__seyal_block__
    // <token>; <command>`) rather than bootstrap-prefixed. Capture `$?` as
    // the first statement in precmd before any conditional can clobber it.
    format!(
        "if (( ! $+functions[_seyal_block_precmd] )); then \
         autoload -Uz add-zsh-hook; \
         _seyal_active_token=; \
         __seyal_block__() {{ _seyal_active_token=$1; printf '\\033]133;C;%s\\007' \"$1\"; }}; \
         _seyal_block_precmd() {{ local _seyal_status=$?; if [[ -n \"$_seyal_active_token\" ]]; then \
         printf '\\033]133;D;%s;%s\\007' \"$_seyal_active_token\" \"$_seyal_status\"; \
         _seyal_active_token=; fi }}; \
         add-zsh-hook precmd _seyal_block_precmd; \
         fi; \
         __seyal_block__ {token_hex}; {command}"
    )
}

/// Result of a Pass 7.1 composer admission attempt. Busy is a correlated
/// application result, not a transport failure, so the Pane keeps its draft
/// and remains connected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg(target_os = "macos")]
pub(crate) enum ComposerAdmission {
    Accepted(CommandBlockId),
    Busy,
    Unsupported,
}

impl Runtime {
    /// Admit one complete Pane-composer command. This deliberately uses a
    /// distinct Runtime operation from raw terminal input: only a trusted
    /// OSC-133 start event can turn this pending metadata into a Running Block.
    #[cfg(target_os = "macos")]
    pub(crate) fn submit_composer_command(
        &mut self,
        id: ExecutionId,
        command: String,
    ) -> Result<ComposerAdmission, RuntimeError> {
        if command.is_empty() || command.len() > MAX_COMMAND_BYTES {
            return Err(RuntimeError::CapacityExceeded);
        }
        let can_admit = self.entries.get(&id).is_some_and(|entry| {
            entry.terminal_io_active()
                && entry.pending_composer_commands.is_empty()
                && entry.active_block.is_none()
        });
        if !can_admit {
            return Ok(ComposerAdmission::Busy);
        }
        let mode = self
            .entries
            .get(&id)
            .map(|entry| entry.shell_integration_mode)
            .ok_or(RuntimeError::UnknownExecution)?;
        if mode == ShellIntegrationMode::Unsupported {
            // Unsupported shells remain fully usable through the ordinary raw
            // PTY path, but never receive synthetic Block metadata.
            let mut bytes = Vec::with_capacity(command.len() + 1);
            bytes.extend_from_slice(command.as_bytes());
            bytes.push(b'\r');
            self.input_ingress(id)?.try_submit(bytes)?;
            return Ok(ComposerAdmission::Unsupported);
        }
        let token = issue_shell_integration_token()?;
        let wrapped = zsh_composer_command(&command, token);
        let mut bytes = Vec::with_capacity(wrapped.len() + 1);
        bytes.extend_from_slice(wrapped.as_bytes());
        bytes.push(b'\r');
        self.input_ingress(id)?.try_submit(bytes)?;
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        let cursor = entry.execution.terminal().cursor();
        let start_line = entry
            .execution
            .terminal()
            .line_id(cursor.row)
            .map(|line| line.0)
            .unwrap_or(1);
        let block_id = entry
            .block_timeline
            .allocate_id()
            .map_err(|_| RuntimeError::CapacityExceeded)?;
        // Pending only: Running Block metadata is published after trusted C.
        entry
            .pending_composer_commands
            .push_back(PendingComposerCommand {
                token,
                command,
                block_id,
                start_line,
            });
        Ok(ComposerAdmission::Accepted(block_id))
    }

    /// Consume bounded canonical parser events after their bytes were applied
    /// to TerminalState. The Runtime records only trusted anchors; this path
    /// never reads a prompt, row text, or terminal cell payload.
    #[cfg(target_os = "macos")]
    pub(super) fn observe_shell_integration_events(
        &mut self,
        id: ExecutionId,
    ) -> Result<(), RuntimeError> {
        let mut changed = false;
        {
            let entry = self
                .entries
                .get_mut(&id)
                .ok_or(RuntimeError::UnknownExecution)?;
            while let Some(event) = entry.execution.take_shell_integration_event() {
                let cursor = entry.execution.terminal().cursor();
                let Some(line_id) = entry.execution.terminal().line_id(cursor.row) else {
                    continue;
                };
                match event {
                    ShellIntegrationEvent::CommandStarted { token } => {
                        let Some(position) = entry
                            .pending_composer_commands
                            .iter()
                            .position(|pending| pending.token == token)
                        else {
                            // Direct/raw shell input remains intentionally
                            // unblocked and produces no guessed Block.
                            continue;
                        };
                        let pending = entry
                            .pending_composer_commands
                            .remove(position)
                            .expect("pending composer position remains valid");
                        if entry
                            .block_timeline
                            .start(pending.block_id, pending.command, pending.start_line)
                            .is_err()
                        {
                            // Admission already succeeded on the PTY; keep the
                            // shell usable but do not publish stale Running state.
                            continue;
                        }
                        entry.active_block = Some(pending.block_id);
                        entry.active_block_token = Some(pending.token);
                        changed = true;
                    }
                    ShellIntegrationEvent::CommandFinished { token, exit_status } => {
                        let Some(block_id) = entry.active_block else {
                            continue;
                        };
                        if entry.active_block_token != Some(token) {
                            continue;
                        }
                        if entry
                            .block_timeline
                            .complete(block_id, line_id.0, exit_status)
                            .is_ok()
                        {
                            entry.active_block = None;
                            entry.active_block_token = None;
                            changed = true;
                        }
                    }
                }
            }
            if changed {
                entry.block_revision = entry.block_revision.saturating_add(1);
            }
        }
        if changed {
            self.publish_block_timeline(id);
        }
        Ok(())
    }

    /// Non-macOS runtimes do not expose the local composer/block route, but
    /// still drain parser events so a raw execution cannot retain a bounded
    /// queue of shell-integration notifications indefinitely.
    #[cfg(not(target_os = "macos"))]
    pub(super) fn observe_shell_integration_events(
        &mut self,
        id: ExecutionId,
    ) -> Result<(), RuntimeError> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(RuntimeError::UnknownExecution)?;
        while entry.execution.take_shell_integration_event().is_some() {}
        Ok(())
    }
}
