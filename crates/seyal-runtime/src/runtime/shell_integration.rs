#[cfg(all(test, target_os = "macos"))]
use seyal_exec::CommandSpec;
#[cfg(target_os = "macos")]
use seyal_exec::{CommandSpec, ShellIntegrationEvent, ShellIntegrationToken};

#[cfg(target_os = "macos")]
use crate::command_block_timeline::{CommandBlockId, MAX_COMMAND_BYTES};
use crate::{ExecutionId, RuntimeError};

use super::Runtime;
#[cfg(target_os = "macos")]
use super::entry::PendingComposerCommand;

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
    format!(
        "if (( ! $+functions[_seyal_block_preexec] )); then autoload -Uz add-zsh-hook; _seyal_active_token=; _seyal_block_preexec() {{ if [[ \"$1\" == __seyal_block__\\ * ]]; then local _seyal_marker=${{1#* }}; _seyal_marker=${{_seyal_marker%%[;\\n]*}}; _seyal_active_token=$_seyal_marker; printf '\\033]133;C;%s\\007' \"$_seyal_active_token\"; fi }}; _seyal_block_precmd() {{ if [[ -n \"$_seyal_active_token\" ]]; then local _seyal_status=$?; printf '\\033]133;D;%s;%s\\007' \"$_seyal_active_token\" \"$_seyal_status\"; _seyal_active_token=; fi }}; add-zsh-hook preexec _seyal_block_preexec; add-zsh-hook precmd _seyal_block_precmd; __seyal_block__() {{ :; }}; fi; __seyal_block__ {token_hex}; {command}"
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
    /// OSC-133 start event can turn this pending metadata into a Block.
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
            .start(command.clone(), start_line)
            .map_err(|_| RuntimeError::CapacityExceeded)?;
        entry.active_block = Some(block_id);
        // The control admission above is bounded and synchronous. Record only
        // after success so rejected composer input cannot manufacture a Block.
        entry
            .pending_composer_commands
            .push_back(PendingComposerCommand::new(token, &command));
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
                        if entry.active_block.is_some() {
                            entry.active_block_token = Some(pending.token);
                            changed = true;
                        }
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
