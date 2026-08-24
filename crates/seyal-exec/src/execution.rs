//! TerminalExecution composition boundary.
//!
//! The implemented type will own one terminal endpoint/PTY, one child
//! lifecycle and one authoritative `seyal_terminal::TerminalState`. It must
//! not create a second VT/grid or move execution ownership into the GUI.
