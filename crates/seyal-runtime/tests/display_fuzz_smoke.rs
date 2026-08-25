use std::{env, fs, path::PathBuf};

use seyal_exec::{
    ProjectionAttributes, ProjectionCell, ProjectionColor, ProjectionDamage,
    TerminalProjectionSnapshot,
};
use seyal_runtime::display::{self, DisplayError};

fn input() -> Vec<u8> {
    let path =
        PathBuf::from(env::var_os("SEYAL_FUZZ_INPUT").expect("SEYAL_FUZZ_INPUT is required"));
    fs::read(path).expect("read retained fuzz seed")
}

fn valid_frame() -> Vec<u8> {
    let cells = vec![
        ProjectionCell {
            scalar: 'A',
            foreground: ProjectionColor::Rgb {
                r: 0x12,
                g: 0x34,
                b: 0x56,
            },
            background: ProjectionColor::Indexed(7),
            attributes: ProjectionAttributes {
                bold: true,
                underline: true,
                inverse: false,
            },
        };
        8
    ];
    let snapshot = TerminalProjectionSnapshot {
        rows: 2,
        columns: 4,
        cursor_row: 1,
        cursor_col: 3,
        cursor_visible: true,
        alternate_screen: false,
        source_damage_generation: 17,
        damage: ProjectionDamage::full(2),
        cells,
    };
    let batch = display::encode_snapshot(&snapshot).expect("valid seed frame");
    assert_eq!(batch.frames.len(), 1);
    batch.frames[0].to_vec()
}

#[test]
#[ignore = "executed by fuzz/targets/display-binary-decode with retained mutation seeds"]
fn display_binary_decode_seed() {
    let controls = input();
    let baseline = valid_frame();
    assert!(display::decode_chunk(&baseline).is_ok());

    // Arbitrary raw bytes always reach the production decoder.
    let _ = display::decode_chunk(&controls);

    // Each seed also mutates a structurally valid production frame so short or
    // textual seeds still exercise deep geometry/generation/color/Unicode/
    // attribute/chunk validation rather than only the magic/header checks.
    for mutation in controls.chunks(3).take(4096) {
        if mutation.is_empty() {
            continue;
        }
        let mut candidate = baseline.clone();
        let index = ((mutation[0] as usize) << 8 | mutation.get(1).copied().unwrap_or(0) as usize)
            % candidate.len();
        let xor = mutation.get(2).copied().unwrap_or(0xff);
        candidate[index] ^= xor;
        let result = display::decode_chunk(&candidate);
        if let Err(error) = result {
            assert!(matches!(
                error,
                DisplayError::InvalidGeometry
                    | DisplayError::InvalidCursor
                    | DisplayError::InvalidDamage
                    | DisplayError::InvalidChunk
                    | DisplayError::InvalidLength
                    | DisplayError::InvalidCell
                    | DisplayError::InvalidColor
                    | DisplayError::InvalidAttributes
                    | DisplayError::InvalidUnicode
                    | DisplayError::WrongMessageType
                    | DisplayError::GenerationMismatch
                    | DisplayError::DimensionMismatch
                    | DisplayError::IncompleteBatch
                    | DisplayError::BatchTooLarge
                    | DisplayError::Overflow
            ));
        }
    }
}