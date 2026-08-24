//! PTY endpoint ownership and master-descriptor encapsulation.
//!
//! The implementation belongs here only after spawn/open semantics are fixed
//! by SPEC-002 and covered by real macOS tests. Raw descriptors must not become
//! an arbitrary public escape hatch.
