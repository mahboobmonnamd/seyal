//! Child-process, session/process-group, wait/reap and exit-status lifecycle.
//!
//! Detach is not represented by dropping this ownership object. Explicit
//! termination and natural child exit are separate lifecycle transitions.
