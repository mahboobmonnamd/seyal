#![cfg(target_os = "macos")]
#![allow(unsafe_code)]

use std::{
    io::{Read, Write},
    os::fd::{AsRawFd, RawFd},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

use seyal_exec::{CommandSpec, WindowSize};
use seyal_runtime::{
    ExecutionId, LocalIpcMode, Runtime, RuntimeConfig,
    display::{decode_chunk, empty_cache},
    local_ipc::framing::{
        Attach, Attached, ClientHello, FrameHeader, HEADER_LEN, MessageType, Role, ServerHello,
        encode_frame,
    },
};

fn config() -> RuntimeConfig {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut config = RuntimeConfig::m001().unwrap();
    config.singleton_path = std::env::temp_dir().join(format!("s5c-{suffix:x}.lock"));
    config.local_ipc = LocalIpcMode::Enabled {
        runtime_dir_override: Some(std::env::temp_dir().join(format!("s5cd-{suffix:x}"))),
    };
    config.graceful_termination = Duration::from_millis(50);
    config.forced_reap = Duration::from_millis(250);
    config.final_drain = Duration::from_millis(50);
    config
}

fn fd_count() -> usize {
    // Avoid /dev/fd enumeration changing the count while it is measured.
    (0..1024)
        .filter(|fd| {
            // SAFETY: F_GETFD only inspects the integer descriptor; invalid
            // descriptors are reported with EBADF and are not modified.
            (unsafe { libc::fcntl(*fd, libc::F_GETFD) }) >= 0
        })
        .count()
}

fn send_many_fds(socket: RawFd, payload: &[u8], fd: RawFd, count: usize) {
    // This deliberately exceeds the old fixed receive scratch capacity. The
    // production receiver must now absorb the complete SCM_RIGHTS set, reject
    // it as protocol-fatal, and close every descriptor without MSG_CTRUNC.
    assert!(count >= 64, "test must exercise a wide SCM_RIGHTS set");
    let payload_bytes = count * std::mem::size_of::<RawFd>();
    // SAFETY: CMSG_SPACE/CMSG_LEN only calculate integer sizes.
    let control_len = unsafe { libc::CMSG_SPACE(payload_bytes as u32) as usize };

    let mut control = [0usize; 128];
    assert!(control_len <= std::mem::size_of_val(&control));
    let mut iov = libc::iovec {
        iov_base: payload.as_ptr() as *mut libc::c_void,
        iov_len: payload.len(),
    };
    // SAFETY: a zeroed msghdr is a valid empty base and every field consumed
    // by sendmsg is initialized below.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast::<libc::c_void>();
    message.msg_controllen = control_len as _;

    // SAFETY: the aligned control buffer has CMSG_SPACE for `count` RawFd
    // values and remains live through sendmsg.
    let cmsg = unsafe { libc::CMSG_FIRSTHDR(&message) };
    assert!(!cmsg.is_null());
    unsafe {
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(payload_bytes as u32) as _;
        for index in 0..count {
            std::ptr::write_unaligned(
                libc::CMSG_DATA(cmsg)
                    .cast::<u8>()
                    .add(index * std::mem::size_of::<RawFd>())
                    .cast::<RawFd>(),
                fd,
            );
        }
        let sent = libc::sendmsg(socket, &message, 0);
        assert!(
            sent >= 0,
            "sendmsg failed: {}",
            std::io::Error::last_os_error()
        );
    }
}

struct Client {
    stream: UnixStream,
    buffered: Vec<u8>,
}

struct Harness {
    runtime: Runtime,
}

impl Harness {
    fn new() -> Self {
        Self {
            runtime: Runtime::new(config()).unwrap(),
        }
    }

    fn pump(&mut self) {
        self.runtime
            .poll_once(Some(Duration::from_millis(5)))
            .unwrap();
    }

    fn spawn_cat(&mut self) -> ExecutionId {
        self.runtime
            .create_execution(
                CommandSpec::new("/bin/cat"),
                WindowSize::new(80, 24, 0, 0).unwrap(),
            )
            .unwrap()
    }

    fn connect(&mut self) -> Client {
        let path = self.runtime.local_ipc_socket_path().unwrap().to_path_buf();
        let stream = UnixStream::connect(path).unwrap();
        stream.set_nonblocking(true).unwrap();
        self.pump();
        Client {
            stream,
            buffered: Vec::new(),
        }
    }

    fn send(&mut self, client: &mut Client, kind: MessageType, payload: &[u8]) {
        client
            .stream
            .write_all(&encode_frame(kind, payload))
            .unwrap();
        self.pump();
    }

    fn frame(&mut self, client: &mut Client) -> (MessageType, Vec<u8>) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if client.buffered.len() >= HEADER_LEN {
                let header = FrameHeader::decode(&client.buffered[..HEADER_LEN]).unwrap();
                let total = HEADER_LEN + header.payload_len as usize;
                if client.buffered.len() >= total {
                    let raw = client.buffered.drain(..total).collect::<Vec<_>>();
                    return (
                        MessageType::from_u16(header.message_type).unwrap(),
                        raw[HEADER_LEN..].to_vec(),
                    );
                }
            }
            let mut chunk = [0u8; 8192];
            match client.stream.read(&mut chunk) {
                Ok(0) => panic!("connection closed"),
                Ok(n) => client.buffered.extend_from_slice(&chunk[..n]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => self.pump(),
                Err(error) => panic!("read failed: {error}"),
            }
            assert!(Instant::now() < deadline, "frame timed out");
        }
    }

    fn hello(&mut self, client: &mut Client) {
        self.send(
            client,
            MessageType::ClientHello,
            &ClientHello {
                client_capabilities: 0,
            }
            .encode(),
        );
        let (kind, payload) = self.frame(client);
        assert_eq!(kind, MessageType::ServerHello);
        ServerHello::decode(&payload).unwrap();
    }

    fn attach_controller(&mut self, client: &mut Client, execution_id: ExecutionId) -> Attached {
        self.send(
            client,
            MessageType::Attach,
            &Attach {
                execution_id,
                requested_role: Role::Controller,
            }
            .encode(),
        );
        let (kind, payload) = self.frame(client);
        assert_eq!(kind, MessageType::Attached);
        let attached = Attached::decode(&payload).unwrap();

        // Consume and validate the initial snapshot so the attach transaction
        // is fully committed before injecting malformed ancillary data.
        let (kind, payload) = self.frame(client);
        assert_eq!(kind, MessageType::DisplaySnapshot);
        let first = decode_chunk(&encode_frame(kind, &payload)).unwrap();
        let expected = first.chunk_count as usize;
        let mut chunks = vec![first];
        while chunks.len() < expected {
            let (next_kind, next_payload) = self.frame(client);
            assert_eq!(next_kind, MessageType::DisplaySnapshot);
            chunks.push(decode_chunk(&encode_frame(next_kind, &next_payload)).unwrap());
        }
        let mut cache = empty_cache();
        cache.apply_chunks(&chunks).unwrap();
        attached
    }
}

#[test]
fn wide_scm_rights_set_is_fatal_closes_all_fds_and_releases_controller() {
    let baseline_fds = fd_count();
    {
        let mut harness = Harness::new();
        let execution_id = harness.spawn_cat();
        let mut owner = harness.connect();
        harness.hello(&mut owner);
        let _controller = harness.attach_controller(&mut owner, execution_id);
        assert_eq!(
            harness
                .runtime
                .lookup(execution_id)
                .unwrap()
                .attachment_count,
            1
        );

        let dev_null = std::fs::File::open("/dev/null").unwrap();
        send_many_fds(owner.stream.as_raw_fd(), b"x", dev_null.as_raw_fd(), 80);

        let deadline = Instant::now() + Duration::from_secs(2);
        while harness
            .runtime
            .lookup(execution_id)
            .unwrap()
            .attachment_count
            != 0
        {
            harness.pump();
            assert!(
                Instant::now() < deadline,
                "illegal SCM_RIGHTS input did not close the connection and release authority"
            );
        }

        // A fresh connection can immediately acquire the controller lease,
        // proving no authority survived the fatal ancillary error.
        let mut replacement = harness.connect();
        harness.hello(&mut replacement);
        let _replacement_controller = harness.attach_controller(&mut replacement, execution_id);

        drop(replacement);
        drop(owner);
        drop(dev_null);
        for _ in 0..4 {
            harness.pump();
        }
        harness.runtime.begin_shutdown().unwrap();
        harness
            .runtime
            .run_until_empty(Instant::now() + Duration::from_secs(3))
            .unwrap();
    }

    assert_eq!(
        fd_count(),
        baseline_fds,
        "descriptor count must return to the pre-test baseline"
    );
}
