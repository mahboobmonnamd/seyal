use std::time::Duration;

use crate::{ExecError, TerminalExecution};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RegistrationToken(u64);

impl RegistrationToken {
    #[cfg(any(target_os = "macos", test))]
    const fn new(slot: u32, generation: u32) -> Self {
        Self(((generation as u64) << 32) | (slot as u64 + 1))
    }

    #[cfg(any(target_os = "macos", test))]
    fn decode(self) -> (usize, u32) {
        (
            ((self.0 as u32).saturating_sub(1)) as usize,
            (self.0 >> 32) as u32,
        )
    }

    pub fn opaque_value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReactorEventKind {
    Readable,
    Writable,
    PrimaryExited,
    Control,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReactorEvent {
    pub token: Option<RegistrationToken>,
    pub kind: ReactorEventKind,
    pub hangup: bool,
}

impl ReactorEvent {
    pub const EMPTY: Self = Self {
        token: None,
        kind: ReactorEventKind::Control,
        hangup: false,
    };
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
pub struct ReactorWaker {
    kqueue: crate::platform::KqueueHandle,
}

#[cfg(target_os = "macos")]
impl ReactorWaker {
    pub fn wake(&self) -> Result<(), ExecError> {
        crate::platform::trigger_control(&self.kqueue)
    }
}

#[cfg(not(target_os = "macos"))]
#[derive(Clone, Default)]
pub struct ReactorWaker;

#[cfg(not(target_os = "macos"))]
impl ReactorWaker {
    pub fn wake(&self) -> Result<(), ExecError> {
        Err(ExecError::UnsupportedPlatform(
            "ExecutionReactor is implemented for macOS only in M001",
        ))
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug)]
struct Slot {
    generation: u32,
    active: bool,
    fd: i32,
    pid: i32,
    writable: bool,
}

#[cfg(target_os = "macos")]
impl Slot {
    const fn vacant() -> Self {
        Self {
            generation: 1,
            active: false,
            fd: -1,
            pid: -1,
            writable: false,
        }
    }
}

#[cfg(target_os = "macos")]
pub struct ExecutionReactor {
    kqueue: crate::platform::KqueueHandle,
    native_buffer: crate::platform::NativeEventBuffer,
    native_events: [crate::platform::NativeEvent; crate::platform::MAX_NATIVE_EVENTS],
    slots: Vec<Slot>,
}

#[cfg(target_os = "macos")]
impl ExecutionReactor {
    pub fn new() -> Result<Self, ExecError> {
        Ok(Self {
            kqueue: crate::platform::create_kqueue()?,
            native_buffer: crate::platform::NativeEventBuffer::new(),
            native_events: [crate::platform::NativeEvent {
                token: 0,
                filter: crate::platform::NativeFilter::Other,
                hangup: false,
            }; crate::platform::MAX_NATIVE_EVENTS],
            slots: Vec::new(),
        })
    }

    pub fn waker(&self) -> ReactorWaker {
        ReactorWaker {
            kqueue: self.kqueue.clone(),
        }
    }

    pub fn register(
        &mut self,
        execution: &TerminalExecution,
    ) -> Result<RegistrationToken, ExecError> {
        self.register_identifiers(execution.reactor_fd(), execution.child_id() as i32)
    }

    fn register_identifiers(&mut self, fd: i32, pid: i32) -> Result<RegistrationToken, ExecError> {
        let token = self.allocate_slot(fd, pid);
        if let Err(error) = crate::platform::register_read(&self.kqueue, fd, token.0) {
            self.release_slot(token);
            return Err(error);
        }
        if let Err(error) = crate::platform::register_process_exit(&self.kqueue, pid, token.0) {
            let _ = crate::platform::deregister_read(&self.kqueue, fd);
            self.release_slot(token);
            return Err(error);
        }
        Ok(token)
    }

    pub fn set_writable(
        &mut self,
        token: RegistrationToken,
        enabled: bool,
    ) -> Result<(), ExecError> {
        let index = self.live_index(token)?;
        let slot = self.slots[index];
        if slot.writable == enabled {
            return Ok(());
        }
        if enabled {
            crate::platform::register_write(&self.kqueue, slot.fd, token.0)?;
        } else {
            crate::platform::deregister_write(&self.kqueue, slot.fd)?;
        }
        self.slots[index].writable = enabled;
        Ok(())
    }

    pub fn deregister(&mut self, token: RegistrationToken) -> Result<(), ExecError> {
        let Some(index) = self.current_index(token) else {
            return Ok(());
        };
        let slot = self.slots[index];
        let mut first_error = None;
        if slot.writable
            && let Err(error) = crate::platform::deregister_write(&self.kqueue, slot.fd)
        {
            first_error = Some(error);
        }
        if let Err(error) = crate::platform::deregister_read(&self.kqueue, slot.fd)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
        if let Err(error) = crate::platform::deregister_process_exit(&self.kqueue, slot.pid)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
        self.release_slot(token);
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub fn wait(
        &mut self,
        output: &mut [ReactorEvent],
        timeout: Option<Duration>,
    ) -> Result<usize, ExecError> {
        let count = crate::platform::wait_events(
            &self.kqueue,
            &mut self.native_buffer,
            timeout,
            &mut self.native_events,
        )?;
        let mut written = 0usize;
        for native in self.native_events[..count].iter().copied() {
            if written == output.len() {
                break;
            }
            if native.filter == crate::platform::NativeFilter::Control {
                output[written] = ReactorEvent {
                    token: None,
                    kind: ReactorEventKind::Control,
                    hangup: false,
                };
                written += 1;
                continue;
            }
            let token = RegistrationToken(native.token);
            if self.current_index(token).is_none() {
                continue;
            }
            let kind = match native.filter {
                crate::platform::NativeFilter::Read => ReactorEventKind::Readable,
                crate::platform::NativeFilter::Write => ReactorEventKind::Writable,
                crate::platform::NativeFilter::ProcessExit => ReactorEventKind::PrimaryExited,
                crate::platform::NativeFilter::Control | crate::platform::NativeFilter::Other => {
                    continue;
                }
            };
            output[written] = ReactorEvent {
                token: Some(token),
                kind,
                hangup: native.hangup,
            };
            written += 1;
        }
        Ok(written)
    }

    pub fn is_current(&self, token: RegistrationToken) -> bool {
        self.current_index(token).is_some()
    }

    fn allocate_slot(&mut self, fd: i32, pid: i32) -> RegistrationToken {
        if let Some((index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| !slot.active)
        {
            slot.active = true;
            slot.fd = fd;
            slot.pid = pid;
            slot.writable = false;
            return RegistrationToken::new(index as u32, slot.generation);
        }
        let index = self.slots.len();
        let mut slot = Slot::vacant();
        slot.active = true;
        slot.fd = fd;
        slot.pid = pid;
        self.slots.push(slot);
        RegistrationToken::new(index as u32, 1)
    }

    fn live_index(&self, token: RegistrationToken) -> Result<usize, ExecError> {
        self.current_index(token)
            .ok_or(ExecError::StaleRegistrationToken)
    }

    fn current_index(&self, token: RegistrationToken) -> Option<usize> {
        let (index, generation) = token.decode();
        let slot = self.slots.get(index)?;
        (slot.active && slot.generation == generation).then_some(index)
    }

    fn release_slot(&mut self, token: RegistrationToken) {
        let (index, generation) = token.decode();
        let Some(slot) = self.slots.get_mut(index) else {
            return;
        };
        if !slot.active || slot.generation != generation {
            return;
        }
        slot.active = false;
        slot.fd = -1;
        slot.pid = -1;
        slot.writable = false;
        slot.generation = slot.generation.wrapping_add(1).max(1);
    }
}

#[cfg(not(target_os = "macos"))]
pub struct ExecutionReactor;

#[cfg(not(target_os = "macos"))]
impl ExecutionReactor {
    pub fn new() -> Result<Self, ExecError> {
        Err(ExecError::UnsupportedPlatform(
            "ExecutionReactor is implemented for macOS only in M001",
        ))
    }

    pub fn waker(&self) -> ReactorWaker {
        ReactorWaker
    }

    pub fn register(
        &mut self,
        _execution: &TerminalExecution,
    ) -> Result<RegistrationToken, ExecError> {
        Err(ExecError::UnsupportedPlatform(
            "ExecutionReactor is implemented for macOS only in M001",
        ))
    }

    pub fn set_writable(
        &mut self,
        _token: RegistrationToken,
        _enabled: bool,
    ) -> Result<(), ExecError> {
        Err(ExecError::UnsupportedPlatform(
            "ExecutionReactor is implemented for macOS only in M001",
        ))
    }

    pub fn deregister(&mut self, _token: RegistrationToken) -> Result<(), ExecError> {
        Ok(())
    }

    pub fn wait(
        &mut self,
        _output: &mut [ReactorEvent],
        _timeout: Option<Duration>,
    ) -> Result<usize, ExecError> {
        Err(ExecError::UnsupportedPlatform(
            "ExecutionReactor is implemented for macOS only in M001",
        ))
    }

    pub fn is_current(&self, _token: RegistrationToken) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::RegistrationToken;

    #[test]
    fn registration_token_round_trips_slot_and_generation() {
        let token = RegistrationToken::new(17, 42);
        assert_eq!(token.decode(), (17, 42));
        assert_ne!(token.opaque_value(), 17);
    }

    #[cfg(target_os = "macos")]
    mod macos {
        use std::time::Duration;

        use crate::{
            CommandSpec, ExecError, ExecutionReactor, ReactorEvent, ReactorEventKind,
            TerminalExecution, TerminationPolicy, WindowSize,
        };

        fn spawn_sleep() -> TerminalExecution {
            TerminalExecution::spawn(
                &CommandSpec::new("/bin/sh").args(["-c", "sleep 30"]),
                WindowSize::new(80, 24, 0, 0).unwrap(),
            )
            .unwrap()
        }

        fn cleanup(execution: &mut TerminalExecution) {
            let _ = execution.terminate(TerminationPolicy::new(
                Duration::from_millis(50),
                Duration::from_secs(1),
            ));
        }

        #[test]
        fn evfilt_user_wakes_idle_reactor() {
            let mut reactor = ExecutionReactor::new().unwrap();
            reactor.waker().wake().unwrap();
            let mut events = [ReactorEvent::EMPTY; 8];
            let count = reactor
                .wait(&mut events, Some(Duration::from_secs(1)))
                .unwrap();
            assert!(
                events[..count]
                    .iter()
                    .any(|event| event.kind == ReactorEventKind::Control)
            );
        }

        #[test]
        fn released_generation_is_stale_before_slot_reuse() {
            let mut reactor = ExecutionReactor::new().unwrap();
            let mut first = spawn_sleep();
            let first_token = reactor.register(&first).unwrap();
            reactor.deregister(first_token).unwrap();
            assert!(!reactor.is_current(first_token));
            assert!(matches!(
                reactor.set_writable(first_token, true),
                Err(ExecError::StaleRegistrationToken)
            ));
            cleanup(&mut first);
        }

        #[test]
        fn slot_reuse_changes_generation_and_rejects_old_token() {
            let mut reactor = ExecutionReactor::new().unwrap();
            let mut first = spawn_sleep();
            let first_token = reactor.register(&first).unwrap();
            reactor.deregister(first_token).unwrap();
            cleanup(&mut first);

            let mut second = spawn_sleep();
            let second_token = reactor.register(&second).unwrap();
            assert_ne!(first_token, second_token);
            assert!(reactor.is_current(second_token));
            assert!(!reactor.is_current(first_token));
            assert!(matches!(
                reactor.set_writable(first_token, true),
                Err(ExecError::StaleRegistrationToken)
            ));
            reactor.deregister(second_token).unwrap();
            cleanup(&mut second);
        }

        #[test]
        fn partial_process_registration_failure_rolls_back_slot_and_read_interest() {
            let mut reactor = ExecutionReactor::new().unwrap();
            let mut execution = spawn_sleep();
            let fd = execution.reactor_fd();
            assert!(reactor.register_identifiers(fd, i32::MAX).is_err());
            assert!(reactor.slots.iter().all(|slot| !slot.active));

            let token = reactor.register(&execution).unwrap();
            assert!(reactor.is_current(token));
            reactor.deregister(token).unwrap();
            cleanup(&mut execution);
        }

        #[test]
        fn writable_interest_can_be_disarmed_when_queue_becomes_empty() {
            let mut reactor = ExecutionReactor::new().unwrap();
            let mut execution = spawn_sleep();
            let token = reactor.register(&execution).unwrap();
            reactor.set_writable(token, true).unwrap();
            reactor.set_writable(token, false).unwrap();
            let mut events = [ReactorEvent::EMPTY; 8];
            let count = reactor
                .wait(&mut events, Some(Duration::from_millis(20)))
                .unwrap();
            assert!(!events[..count].iter().any(|event| {
                event.token == Some(token) && event.kind == ReactorEventKind::Writable
            }));
            reactor.deregister(token).unwrap();
            cleanup(&mut execution);
        }
    }
}
