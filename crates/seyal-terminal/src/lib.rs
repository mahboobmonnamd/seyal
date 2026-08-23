//! Seyal-owned terminal semantics.
//!
//! This crate is the single portable authority for incremental VT parsing and
//! canonical terminal state. It owns no PTY, runtime IPC, renderer or native UI
//! behavior. RILL is historical implementation evidence only; Seyal semantics
//! are defined by the current architecture, M001 specification and tests.

mod cell;
mod color;
mod cursor;
mod damage;
mod error;
mod line;
mod modes;
mod parser;
mod screen;
mod style;
mod terminal;

pub use cell::Cell;
pub use color::Color;
pub use cursor::CursorState;
pub use damage::Damage;
pub use error::TerminalError;
pub use line::LineId;
pub use modes::ModeState;
pub use style::Style;
pub use terminal::{Diagnostics, TerminalState};
