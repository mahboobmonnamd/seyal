use crate::{
    grapheme_store::{StoreAdmit, INLINE_STORE_ID},
    Color, Style,
};

/// Physical-cell role in the canonical grid (SPEC-011).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum CellRole {
    #[default]
    Empty = 0,
    Lead = 1,
    Continuation = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// Primary scalar for M001 Candidate-D projection until #817.
    ///
    /// Lead: first scalar of the grapheme, or U+FFFD when the payload is the
    /// overflow sentinel. Empty/Continuation: space (no independent text).
    pub character: char,
    pub style: Style,
    pub role: CellRole,
    /// Terminal occupation for Lead (1 or 2). Empty and Continuation use 0.
    pub width: u8,
    /// Grapheme-store id for multi-scalar payload; [`INLINE_STORE_ID`] if inline.
    pub(crate) store_id: u32,
    /// When true, copy/search must treat payload as U+FFFD (SPEC-011 §9).
    pub(crate) overflow: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self::blank(Color::Default)
    }
}

impl Cell {
    pub(crate) fn blank(background: Color) -> Self {
        Self {
            character: ' ',
            style: Style {
                bg: background,
                ..Style::default()
            },
            role: CellRole::Empty,
            width: 0,
            store_id: INLINE_STORE_ID,
            overflow: false,
        }
    }

    pub(crate) fn lead_inline(character: char, width: u8, style: Style) -> Self {
        Self {
            character,
            style,
            role: CellRole::Lead,
            width,
            store_id: INLINE_STORE_ID,
            overflow: false,
        }
    }

    pub(crate) fn lead_from_admit(first: char, width: u8, style: Style, admit: StoreAdmit) -> Self {
        match admit {
            StoreAdmit::Inline => Self::lead_inline(first, width, style),
            StoreAdmit::Stored(id) => Self {
                character: first,
                style,
                role: CellRole::Lead,
                width,
                store_id: id,
                overflow: false,
            },
            StoreAdmit::OverflowSentinel => Self {
                character: '\u{FFFD}',
                style,
                role: CellRole::Lead,
                width,
                store_id: INLINE_STORE_ID,
                overflow: true,
            },
        }
    }

    pub(crate) fn continuation() -> Self {
        Self {
            character: ' ',
            style: Style::default(),
            role: CellRole::Continuation,
            width: 0,
            store_id: INLINE_STORE_ID,
            overflow: false,
        }
    }

    pub fn is_continuation(self) -> bool {
        self.role == CellRole::Continuation
    }

    pub fn is_lead(self) -> bool {
        self.role == CellRole::Lead
    }

    /// Canonical text for copy/search: overflow yields U+FFFD.
    pub fn canonical_scalar(self) -> char {
        if self.overflow {
            '\u{FFFD}'
        } else {
            match self.role {
                CellRole::Lead => self.character,
                CellRole::Empty | CellRole::Continuation => ' ',
            }
        }
    }
}
