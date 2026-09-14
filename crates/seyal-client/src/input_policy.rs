//! Cold SPEC-006 §21.3 input policy. Not visual theme and not RuntimeConfig.

use std::path::{Path, PathBuf};

use crate::theme::{parse_toml, TomlError, TomlValue, ENV_CONFIG};

/// Immutable routing intent consumed by the native keyboard classifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputPolicy {
    pub option_as_alt: bool,
}

impl InputPolicy {
    /// Built-in default: Option stays with AppKit layout/IME.
    pub const DEFAULT: Self = Self {
        option_as_alt: false,
    };
}

/// Load `[input] option_as_alt` from TOML text. Missing, unreadable, or invalid
/// values retain false. Unknown input keys are diagnosed and ignored.
pub fn load_input_policy(toml_text: Option<&str>) -> (InputPolicy, Vec<String>) {
    let mut policy = InputPolicy::DEFAULT;
    let mut warnings = Vec::new();
    let Some(text) = toml_text else {
        return (policy, warnings);
    };
    let root = match parse_toml(text) {
        Ok(root) => root,
        Err(TomlError(_)) => return (policy, warnings),
    };
    let Some(input) = root.get("input").and_then(TomlValue::as_table) else {
        return (policy, warnings);
    };
    for key in input.keys() {
        if key != "option_as_alt" {
            warnings.push(format!("unknown key input.{key} ignored"));
        }
    }
    match input.get("option_as_alt") {
        None => {}
        Some(value) => {
            if let Some(parsed) = value.as_bool() {
                policy.option_as_alt = parsed;
            } else {
                warnings.push("input.option_as_alt ignored; expected boolean".into());
            }
        }
    }
    (policy, warnings)
}

/// Same file as visual config: `SEYAL_CONFIG` or `~/.config/seyal/config.toml`.
pub fn input_policy_config_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(ENV_CONFIG) {
        let path = PathBuf::from(path);
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/seyal/config.toml"))
}

pub fn load_input_policy_from_path(path: Option<&Path>) -> (InputPolicy, Vec<String>) {
    let text = path.and_then(|path| std::fs::read_to_string(path).ok());
    load_input_policy(text.as_deref())
}

/// Process-wide immutable result captured once at first native query.
pub fn process_input_policy() -> InputPolicy {
    static POLICY: std::sync::OnceLock<InputPolicy> = std::sync::OnceLock::new();
    *POLICY.get_or_init(|| load_input_policy_from_path(input_policy_config_path().as_deref()).0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_false() {
        assert_eq!(load_input_policy(None).0, InputPolicy::DEFAULT);
    }

    #[test]
    fn true_and_false_from_input_table() {
        assert!(
            load_input_policy(Some("[input]\noption_as_alt = true"))
                .0
                .option_as_alt
        );
        assert!(
            !load_input_policy(Some("[input]\noption_as_alt = false"))
                .0
                .option_as_alt
        );
    }

    #[test]
    fn invalid_and_unknown_retain_false_and_diagnose() {
        let (policy, warnings) = load_input_policy(Some("[input]\noption_as_alt = \"yes\""));
        assert!(!policy.option_as_alt);
        assert_eq!(
            warnings,
            vec!["input.option_as_alt ignored; expected boolean"]
        );

        let (policy, warnings) =
            load_input_policy(Some("[input]\noption_as_alt = true\nextra = 1"));
        assert!(policy.option_as_alt);
        assert_eq!(warnings, vec!["unknown key input.extra ignored"]);

        let (policy, warnings) = load_input_policy(Some("this is not = toml ["));
        assert!(!policy.option_as_alt);
        assert!(warnings.is_empty());
    }

    #[test]
    fn visual_tables_do_not_set_input_policy() {
        let (policy, warnings) = load_input_policy(Some(
            "[ui]\nappearance = \"dark\"\n[input]\noption_as_alt = true",
        ));
        assert!(policy.option_as_alt);
        assert!(warnings.is_empty());
    }
}
