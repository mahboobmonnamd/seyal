//! Nonblocking PTY readiness and bounded read/write coordination.
//!
//! This module will own the poll/readiness mechanics needed by M001. It must
//! not force one thread per PTY, busy-wait, serialize bytes or cross into Swift
//! on the terminal I/O hot path.
