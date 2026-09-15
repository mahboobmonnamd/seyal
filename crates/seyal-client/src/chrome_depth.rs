//! Adaptive Depth projection for thin-host chrome regions.
//!
//! Rust owns which depth role each existing chrome surface receives. The host
//! only realizes the resolved material/color. This module does not create
//! Workspace/Tab/Pane/agent identities — it projects presentation from
//! [`ChromeSnapshot`] + focused-pane [`ComposerSnapshot`] + [`ResolvedVisual`].

use crate::chrome::ChromeSnapshot;
use crate::composer::{ComposerMode, ComposerSnapshot};
use crate::theme::{DepthLevel, ResolvedMaterial, ResolvedVisual};

/// Existing chrome regions that receive Adaptive Depth treatment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChromeSurface {
    Left,
    Inspector,
    TabStrip,
    AttentionPopover,
    PaletteOverlay,
    Composer,
}

impl ChromeSurface {
    pub const ALL: [Self; 6] = [
        Self::Left,
        Self::Inspector,
        Self::TabStrip,
        Self::AttentionPopover,
        Self::PaletteOverlay,
        Self::Composer,
    ];
}

/// One host-facing surface style: depth role + resolved material token.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChromeSurfaceStyle {
    pub surface: ChromeSurface,
    pub depth: DepthLevel,
    pub material: ResolvedMaterial,
}

/// Focus/attention signals used for Adaptive Depth focus gravity.
///
/// Derived from existing chrome/composer bits — not a new product identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChromeDepthSignals {
    pub left_visible: bool,
    pub inspector_visible: bool,
    pub tab_strip_visible: bool,
    pub attention_popover_open: bool,
    pub palette_open: bool,
    pub has_selected_agent: bool,
    pub composer_available: bool,
    pub composer_editing: bool,
}

impl ChromeDepthSignals {
    pub fn from_snapshots(chrome: &ChromeSnapshot, composer: Option<&ComposerSnapshot>) -> Self {
        let (composer_available, composer_editing) = match composer {
            Some(snap) => {
                let available = matches!(snap.mode, ComposerMode::Available);
                let editing = available && (!snap.draft.is_empty() || snap.history_open);
                (available, editing)
            }
            None => (false, false),
        };
        Self {
            left_visible: chrome.left_visible,
            inspector_visible: chrome.inspector_visible,
            tab_strip_visible: chrome.tab_strip_visible,
            attention_popover_open: chrome.attention_popover_open,
            palette_open: chrome.palette_open,
            has_selected_agent: chrome.selected_agent.is_some(),
            composer_available,
            composer_editing,
        }
    }
}

/// Map a chrome surface to its Adaptive Depth level for the current signals.
///
/// Budget (design language): D0 truth stays on the terminal canvas (not chrome);
/// D1 receded utility at rest; D2 active utility under focus gravity; D3 only
/// for attention popover prominence.
pub fn depth_for_surface(surface: ChromeSurface, signals: ChromeDepthSignals) -> DepthLevel {
    match surface {
        ChromeSurface::Left => {
            if !signals.left_visible {
                return DepthLevel::RecededUtility;
            }
            // Recede while overlays own attention; promote while agent context
            // is selected in the left inventory (active navigation).
            if signals.attention_popover_open || signals.palette_open {
                DepthLevel::RecededUtility
            } else if signals.has_selected_agent {
                DepthLevel::ActiveUtility
            } else {
                DepthLevel::RecededUtility
            }
        }
        ChromeSurface::Inspector => {
            if !signals.inspector_visible {
                return DepthLevel::RecededUtility;
            }
            if signals.attention_popover_open || signals.palette_open {
                DepthLevel::RecededUtility
            } else if signals.has_selected_agent {
                DepthLevel::ActiveUtility
            } else {
                DepthLevel::RecededUtility
            }
        }
        ChromeSurface::TabStrip => DepthLevel::RecededUtility,
        ChromeSurface::AttentionPopover => {
            if signals.attention_popover_open {
                DepthLevel::Attention
            } else {
                DepthLevel::RecededUtility
            }
        }
        ChromeSurface::PaletteOverlay => {
            if signals.palette_open {
                DepthLevel::ActiveUtility
            } else {
                DepthLevel::RecededUtility
            }
        }
        ChromeSurface::Composer => {
            if !signals.composer_available {
                DepthLevel::RecededUtility
            } else if signals.attention_popover_open || signals.palette_open {
                // Overlays own focus gravity; composer recedes while they are open.
                DepthLevel::RecededUtility
            } else if signals.composer_editing {
                DepthLevel::ActiveUtility
            } else {
                // Empty available composer stays D1 at rest.
                DepthLevel::RecededUtility
            }
        }
    }
}

/// Resolve the material token a host should apply for one chrome surface.
pub fn material_for_surface(
    visual: &ResolvedVisual,
    surface: ChromeSurface,
    depth: DepthLevel,
) -> ResolvedMaterial {
    // Palette uses Overlay color role while retaining ActiveUtility depth intent.
    if surface == ChromeSurface::PaletteOverlay && depth == DepthLevel::ActiveUtility {
        let base = visual.material(DepthLevel::ActiveUtility);
        return ResolvedMaterial {
            depth,
            intent: base.intent,
            color: visual.colors.get(crate::theme::ColorRole::Overlay).with_alpha(
                if visual.reduce_transparency {
                    1.0
                } else {
                    base.color.alpha.max(0.94)
                },
            ),
        };
    }
    visual.material(depth)
}

/// Project all chrome surfaces through the resolved theme materials.
pub fn project_chrome_surfaces(
    visual: &ResolvedVisual,
    signals: ChromeDepthSignals,
) -> Vec<ChromeSurfaceStyle> {
    ChromeSurface::ALL
        .into_iter()
        .map(|surface| {
            let depth = depth_for_surface(surface, signals);
            let material = material_for_surface(visual, surface, depth);
            ChromeSurfaceStyle {
                surface,
                depth,
                material,
            }
        })
        .collect()
}

/// Host convenience: motion durations from the resolved visual (0 when reduced).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChromeMotionProjection {
    pub allows_motion: bool,
    pub focus_duration: f64,
    pub overlay_duration: f64,
    pub reduce_transparency: bool,
}

impl ChromeMotionProjection {
    pub fn from_visual(visual: &ResolvedVisual) -> Self {
        Self {
            allows_motion: visual.motion.allows_motion,
            focus_duration: visual.motion.focus_duration,
            overlay_duration: visual.motion.overlay_duration,
            reduce_transparency: visual.reduce_transparency,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{canonical, AccessibilitySignals, MaterialIntent, ResolvedAppearance};

    fn visual() -> ResolvedVisual {
        canonical(ResolvedAppearance::Dark, AccessibilitySignals::default())
    }

    fn rest_signals() -> ChromeDepthSignals {
        ChromeDepthSignals {
            left_visible: true,
            inspector_visible: true,
            tab_strip_visible: true,
            attention_popover_open: false,
            palette_open: false,
            has_selected_agent: false,
            composer_available: true,
            composer_editing: false,
        }
    }

    #[test]
    fn resting_chrome_maps_to_receded_utility() {
        let signals = rest_signals();
        assert_eq!(
            depth_for_surface(ChromeSurface::Left, signals),
            DepthLevel::RecededUtility
        );
        assert_eq!(
            depth_for_surface(ChromeSurface::Inspector, signals),
            DepthLevel::RecededUtility
        );
        assert_eq!(
            depth_for_surface(ChromeSurface::TabStrip, signals),
            DepthLevel::RecededUtility
        );
        assert_eq!(
            depth_for_surface(ChromeSurface::Composer, signals),
            DepthLevel::RecededUtility
        );
        assert_eq!(
            depth_for_surface(ChromeSurface::AttentionPopover, signals),
            DepthLevel::RecededUtility
        );
        assert_eq!(
            depth_for_surface(ChromeSurface::PaletteOverlay, signals),
            DepthLevel::RecededUtility
        );
    }

    #[test]
    fn selected_agent_promotes_left_and_inspector_to_active() {
        let mut signals = rest_signals();
        signals.has_selected_agent = true;
        assert_eq!(
            depth_for_surface(ChromeSurface::Left, signals),
            DepthLevel::ActiveUtility
        );
        assert_eq!(
            depth_for_surface(ChromeSurface::Inspector, signals),
            DepthLevel::ActiveUtility
        );
        assert_eq!(
            depth_for_surface(ChromeSurface::TabStrip, signals),
            DepthLevel::RecededUtility
        );
    }

    #[test]
    fn attention_popover_uses_d3_and_recedes_side_chrome() {
        let mut signals = rest_signals();
        signals.attention_popover_open = true;
        signals.has_selected_agent = true;
        assert_eq!(
            depth_for_surface(ChromeSurface::AttentionPopover, signals),
            DepthLevel::Attention
        );
        assert_eq!(
            depth_for_surface(ChromeSurface::Left, signals),
            DepthLevel::RecededUtility
        );
        assert_eq!(
            depth_for_surface(ChromeSurface::Inspector, signals),
            DepthLevel::RecededUtility
        );
    }

    #[test]
    fn palette_overlay_uses_active_utility_and_recedes_composer() {
        let mut signals = rest_signals();
        signals.palette_open = true;
        signals.composer_editing = true;
        assert_eq!(
            depth_for_surface(ChromeSurface::PaletteOverlay, signals),
            DepthLevel::ActiveUtility
        );
        assert_eq!(
            depth_for_surface(ChromeSurface::Composer, signals),
            DepthLevel::RecededUtility
        );
        assert_eq!(
            depth_for_surface(ChromeSurface::Left, signals),
            DepthLevel::RecededUtility
        );
    }

    #[test]
    fn composer_editing_promotes_to_active_utility() {
        let mut signals = rest_signals();
        signals.composer_editing = true;
        assert_eq!(
            depth_for_surface(ChromeSurface::Composer, signals),
            DepthLevel::ActiveUtility
        );
    }

    #[test]
    fn projection_uses_theme_materials_and_respects_reduced_transparency() {
        let frosted = visual();
        let reduced = canonical(
            ResolvedAppearance::Dark,
            AccessibilitySignals {
                reduce_transparency: true,
                reduce_motion: true,
                increase_contrast: false,
            },
        );
        let signals = rest_signals();
        let frosted_styles = project_chrome_surfaces(&frosted, signals);
        let reduced_styles = project_chrome_surfaces(&reduced, signals);
        let left_frosted = frosted_styles
            .iter()
            .find(|s| s.surface == ChromeSurface::Left)
            .unwrap();
        let left_reduced = reduced_styles
            .iter()
            .find(|s| s.surface == ChromeSurface::Left)
            .unwrap();
        assert_eq!(left_frosted.depth, DepthLevel::RecededUtility);
        assert_eq!(left_frosted.material.intent, MaterialIntent::Frosted);
        assert_eq!(left_reduced.material.intent, MaterialIntent::Tonal);
        assert!(left_reduced.material.color.alpha >= 0.99);

        let motion = ChromeMotionProjection::from_visual(&reduced);
        assert!(!motion.allows_motion);
        assert_eq!(motion.focus_duration, 0.0);
        assert!(motion.reduce_transparency);

        let open = ChromeDepthSignals {
            palette_open: true,
            ..signals
        };
        let palette = project_chrome_surfaces(&frosted, open)
            .into_iter()
            .find(|s| s.surface == ChromeSurface::PaletteOverlay)
            .unwrap();
        assert_eq!(palette.depth, DepthLevel::ActiveUtility);
        assert_eq!(palette.material.intent, MaterialIntent::Frosted);
    }

    #[test]
    fn truth_depth_is_never_assigned_to_chrome_surfaces() {
        let visual = visual();
        for signals in [
            rest_signals(),
            ChromeDepthSignals {
                attention_popover_open: true,
                palette_open: true,
                has_selected_agent: true,
                composer_editing: true,
                ..rest_signals()
            },
        ] {
            for style in project_chrome_surfaces(&visual, signals) {
                assert_ne!(style.depth, DepthLevel::Truth);
                assert_ne!(style.material.depth, DepthLevel::Truth);
            }
        }
    }
}
