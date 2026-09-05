use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WidthContract {
    /// DEC private mode 2027 semantics: consecutively printed scalars that do
    /// not break under UAX #29 occupy one terminal grapheme unit whose width is
    /// determined by the terminal Unicode policy.
    UnicodeGrapheme,
    /// Compatibility semantics: non-zero-width scalars are placed
    /// independently; zero-width scalars may decorate the previous compatible
    /// cell. This intentionally does not promise mode-2027 emoji/ZWJ width.
    LegacyScalar,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PlacedUnit {
    text: String,
    width: usize,
}

fn unicode_units(text: &str) -> Vec<PlacedUnit> {
    text.graphemes(true)
        .map(|cluster| PlacedUnit {
            text: cluster.to_owned(),
            width: UnicodeWidthStr::width(cluster),
        })
        .collect()
}

fn legacy_units(text: &str) -> Vec<PlacedUnit> {
    let mut result: Vec<PlacedUnit> = Vec::new();
    for scalar in text.chars() {
        let width = scalar.width().unwrap_or(0);
        if width == 0 {
            if let Some(previous) = result.last_mut() {
                previous.text.push(scalar);
            }
            continue;
        }
        result.push(PlacedUnit {
            text: scalar.to_string(),
            width,
        });
    }
    result
}

fn place(contract: WidthContract, text: &str) -> Vec<PlacedUnit> {
    match contract {
        WidthContract::UnicodeGrapheme => unicode_units(text),
        WidthContract::LegacyScalar => legacy_units(text),
    }
}

fn total_width(units: &[PlacedUnit]) -> usize {
    units.iter().map(|unit| unit.width).sum()
}

pub(crate) fn report_compatibility_boundary() {
    const CASES: &[(&str, &str)] = &[
        ("combining", "e\u{301}"),
        ("vs16", "❤\u{fe0f}"),
        ("zwj", "👩‍💻"),
        ("family", "👨‍👩‍👧‍👦"),
        ("flag", "🇮🇳"),
    ];

    println!("COMPAT\tlabel\tcontract\tunits\ttotal_cells\tmax_unit_cells");
    for (label, text) in CASES {
        for contract in [WidthContract::UnicodeGrapheme, WidthContract::LegacyScalar] {
            let units = place(contract, text);
            let max_width = units.iter().map(|unit| unit.width).max().unwrap_or(0);
            println!(
                "COMPAT\t{label}\t{contract:?}\t{}\t{}\t{}",
                units.len(),
                total_width(&units),
                max_width,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_mode_collapses_family_emoji_to_one_two_cell_unit() {
        let units = place(WidthContract::UnicodeGrapheme, "👨‍👩‍👧‍👦");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].width, 2);
    }

    #[test]
    fn legacy_mode_keeps_nonzero_emoji_scalars_as_independent_cells() {
        let units = place(WidthContract::LegacyScalar, "👨‍👩‍👧‍👦");
        assert_eq!(units.len(), 4);
        assert_eq!(total_width(&units), 8);
        assert!(units.iter().all(|unit| unit.width <= 2));
    }

    #[test]
    fn legacy_zero_width_combining_scalar_decorates_previous_cell() {
        let units = place(WidthContract::LegacyScalar, "e\u{301}");
        assert_eq!(units.len(), 1);
        assert_eq!(units[0].text, "e\u{301}");
        assert_eq!(units[0].width, 1);
    }

    #[test]
    fn vs16_width_change_is_a_unicode_mode_guarantee_not_a_legacy_assumption() {
        let unicode = place(WidthContract::UnicodeGrapheme, "❤\u{fe0f}");
        let legacy = place(WidthContract::LegacyScalar, "❤\u{fe0f}");
        assert_eq!(total_width(&unicode), 2);
        assert_eq!(total_width(&legacy), 1);
    }

    #[test]
    fn both_contracts_can_use_only_one_or_two_cell_physical_occupations() {
        for text in ["A", "界", "e\u{301}", "❤\u{fe0f}", "👩‍💻", "👨‍👩‍👧‍👦", "🇮🇳"]
        {
            for contract in [WidthContract::UnicodeGrapheme, WidthContract::LegacyScalar] {
                let units = place(contract, text);
                assert!(
                    units.iter().all(|unit| (1..=2).contains(&unit.width)),
                    "{contract:?} produced an unsupported physical unit for {text:?}: {units:?}"
                );
            }
        }
    }
}
