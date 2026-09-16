//! Canonical xterm mouse reports. Runtime selects encoding from `ModeState`.

use crate::modes::{ModeState, MouseReporting};

const MAX_REPORT_BYTES: usize = 32;
const X10_COORD_MAX: u16 = 223;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseEventKind {
    Press,
    Release,
    Move,
    Wheel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseReport {
    pub kind: MouseEventKind,
    pub button: u8,
    pub shift: bool,
    pub alt: bool,
    pub control: bool,
    pub col: u16,
    pub row: u16,
}

pub fn mouse_event_admitted(kind: MouseEventKind, button: u8, modes: ModeState) -> bool {
    match modes.mouse_reporting {
        MouseReporting::Off => false,
        MouseReporting::Button => matches!(
            kind,
            MouseEventKind::Press | MouseEventKind::Release | MouseEventKind::Wheel
        ),
        MouseReporting::ButtonDrag => match kind {
            MouseEventKind::Press | MouseEventKind::Release | MouseEventKind::Wheel => true,
            MouseEventKind::Move => button <= 2,
        },
        MouseReporting::Any => true,
    }
}

pub fn encode_mouse_report(report: MouseReport, modes: ModeState) -> Option<Vec<u8>> {
    if !mouse_event_admitted(report.kind, report.button, modes) {
        return None;
    }
    let button = report_button(report)?;
    let col = report.col.checked_add(1)?;
    let row = report.row.checked_add(1)?;
    if modes.mouse_sgr {
        encode_sgr(
            button,
            col,
            row,
            matches!(report.kind, MouseEventKind::Release),
        )
    } else {
        let x10_button = if report.kind == MouseEventKind::Release {
            3 + button.saturating_sub(report.button)
        } else {
            button
        };
        encode_x10(x10_button, col, row)
    }
}

fn report_button(report: MouseReport) -> Option<u8> {
    let mut button = match report.kind {
        MouseEventKind::Press => {
            if report.button > 2 {
                return None;
            }
            report.button
        }
        MouseEventKind::Release => {
            if report.button > 2 {
                return None;
            }
            report.button
        }
        MouseEventKind::Move => {
            let base = if report.button <= 2 {
                report.button
            } else if report.button == 3 {
                3
            } else {
                return None;
            };
            base.checked_add(32)?
        }
        MouseEventKind::Wheel => {
            if !(64..=67).contains(&report.button) {
                return None;
            }
            report.button
        }
    };
    if report.shift {
        button = button.checked_add(4)?;
    }
    if report.alt {
        button = button.checked_add(8)?;
    }
    if report.control {
        button = button.checked_add(16)?;
    }
    Some(button)
}

fn encode_sgr(button: u8, col: u16, row: u16, release: bool) -> Option<Vec<u8>> {
    let mut buf = [0u8; MAX_REPORT_BYTES];
    let mut len = 0usize;
    buf[len] = 0x1b;
    len += 1;
    buf[len] = b'[';
    len += 1;
    buf[len] = b'<';
    len += 1;
    len += write_u16(&mut buf[len..], u16::from(button))?;
    buf[len] = b';';
    len += 1;
    len += write_u16(&mut buf[len..], col)?;
    buf[len] = b';';
    len += 1;
    len += write_u16(&mut buf[len..], row)?;
    buf[len] = if release { b'm' } else { b'M' };
    len += 1;
    Some(buf[..len].to_vec())
}

fn encode_x10(button: u8, col: u16, row: u16) -> Option<Vec<u8>> {
    if col > X10_COORD_MAX || row > X10_COORD_MAX {
        return None;
    }
    Some(vec![
        0x1b,
        b'[',
        b'M',
        32 + button,
        32 + col as u8,
        32 + row as u8,
    ])
}

fn write_u16(out: &mut [u8], value: u16) -> Option<usize> {
    let mut tmp = [0u8; 5];
    let mut n = value;
    let mut i = tmp.len();
    loop {
        i -= 1;
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    let digits = &tmp[i..];
    if out.len() < digits.len() {
        return None;
    }
    out[..digits.len()].copy_from_slice(digits);
    Some(digits.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sgr_modes() -> ModeState {
        ModeState {
            mouse_reporting: MouseReporting::Button,
            mouse_sgr: true,
            ..ModeState::default()
        }
    }

    #[test]
    fn sgr_press_is_one_based() {
        let bytes = encode_mouse_report(
            MouseReport {
                kind: MouseEventKind::Press,
                button: 0,
                shift: false,
                alt: false,
                control: false,
                col: 0,
                row: 0,
            },
            sgr_modes(),
        )
        .unwrap();
        assert_eq!(bytes, b"\x1b[<0;1;1M");
    }

    #[test]
    fn sgr_release_uses_lowercase_m() {
        let bytes = encode_mouse_report(
            MouseReport {
                kind: MouseEventKind::Release,
                button: 2,
                shift: true,
                alt: false,
                control: true,
                col: 9,
                row: 4,
            },
            sgr_modes(),
        )
        .unwrap();
        assert_eq!(bytes, b"\x1b[<22;10;5m");
    }

    #[test]
    fn x10_release_uses_button_three() {
        let modes = ModeState {
            mouse_reporting: MouseReporting::Button,
            mouse_sgr: false,
            ..ModeState::default()
        };
        let bytes = encode_mouse_report(
            MouseReport {
                kind: MouseEventKind::Release,
                button: 0,
                shift: true,
                alt: false,
                control: false,
                col: 2,
                row: 1,
            },
            modes,
        )
        .unwrap();
        assert_eq!(bytes, b"\x1b[M'#\"");
    }

    #[test]
    fn button_mode_swallows_motion() {
        assert!(encode_mouse_report(
            MouseReport {
                kind: MouseEventKind::Move,
                button: 0,
                shift: false,
                alt: false,
                control: false,
                col: 1,
                row: 1,
            },
            sgr_modes(),
        )
        .is_none());
    }
}
