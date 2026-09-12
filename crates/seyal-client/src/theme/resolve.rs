//! Cold theme resolver. Platform appearance and accessibility are inputs;
//! NSFont/NSColor mapping stays in the native host.

use super::config::{AppearancePreference, ConfigurationDiagnostics, UserUiSettings};
use super::tokens::{
    palette_color, typography_specs, AccessibilitySignals, ColorRole, DepthLevel, FontSpec,
    MaterialIntent, Metrics, MotionSettings, ResolvedAppearance, ResolvedFontSpec,
    ResolvedMaterial, SeamRole, Srgb, TypographyRole,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedColors {
    values: Vec<(ColorRole, Srgb)>,
}

impl ResolvedColors {
    pub fn get(&self, role: ColorRole) -> Srgb {
        self.values
            .iter()
            .find(|(candidate, _)| *candidate == role)
            .map(|(_, color)| *color)
            .unwrap_or(Srgb {
                red: 0.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedTypography {
    pub specs: Vec<(TypographyRole, FontSpec)>,
}

impl ResolvedTypography {
    pub fn spec(&self, role: TypographyRole) -> Option<&FontSpec> {
        self.specs
            .iter()
            .find(|(candidate, _)| *candidate == role)
            .map(|(_, spec)| spec)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedVisual {
    pub appearance: ResolvedAppearance,
    pub settings: UserUiSettings,
    pub colors: ResolvedColors,
    pub typography: ResolvedTypography,
    pub metrics: Metrics,
    pub motion: MotionSettings,
    pub materials: Vec<(DepthLevel, ResolvedMaterial)>,
    pub ui_font: ResolvedFontSpec,
    pub terminal_font: ResolvedFontSpec,
    pub reduce_transparency: bool,
    pub diagnostics: ConfigurationDiagnostics,
}

impl ResolvedVisual {
    pub fn material(&self, depth: DepthLevel) -> ResolvedMaterial {
        self.materials
            .iter()
            .find(|(candidate, _)| *candidate == depth)
            .map(|(_, material)| *material)
            .unwrap_or(ResolvedMaterial {
                depth,
                intent: if depth == DepthLevel::Truth {
                    MaterialIntent::Opaque
                } else {
                    MaterialIntent::Tonal
                },
                color: self.colors.get(ColorRole::Container),
            })
    }

    pub fn seam_color(&self, role: SeamRole) -> Srgb {
        match role {
            SeamRole::Rest => self.colors.get(ColorRole::SeamRest),
            SeamRole::Hover => self.colors.get(ColorRole::SeamHover),
            SeamRole::Focus => self.colors.get(ColorRole::SeamFocus),
            SeamRole::Running => self.colors.get(ColorRole::SeamRunning),
            SeamRole::Attention => self.colors.get(ColorRole::SeamAttention),
        }
    }
}

pub fn resolve(
    mut settings: UserUiSettings,
    platform_appearance: ResolvedAppearance,
    accessibility: AccessibilitySignals,
    diagnostics: ConfigurationDiagnostics,
) -> ResolvedVisual {
    settings.clamp();

    let appearance = match settings.appearance {
        AppearancePreference::System => platform_appearance,
        AppearancePreference::Light => ResolvedAppearance::Light,
        AppearancePreference::Dark => ResolvedAppearance::Dark,
    };

    let colors = ResolvedColors {
        values: ColorRole::ALL
            .into_iter()
            .map(|role| {
                (
                    role,
                    palette_color(role, appearance, accessibility.increase_contrast),
                )
            })
            .collect(),
    };

    let ui_font = ResolvedFontSpec {
        family: settings.ui_font_family.clone(),
        fallbacks: settings.ui_font_fallbacks.clone(),
        point_size: settings.ui_font_size,
    };
    let terminal_font = ResolvedFontSpec {
        family: settings.terminal_font_family.clone(),
        fallbacks: settings.terminal_font_fallbacks.clone(),
        point_size: settings.terminal_font_size,
    };
    let typography = ResolvedTypography {
        specs: typography_specs(&ui_font, &terminal_font),
    };

    let frost_allowed = !settings.reduced_material && !accessibility.reduce_transparency;
    let opacity = if frost_allowed {
        settings.utility_opacity
    } else {
        1.0
    };
    let materials = vec![
        (
            DepthLevel::Truth,
            ResolvedMaterial {
                depth: DepthLevel::Truth,
                intent: MaterialIntent::Opaque,
                color: colors.get(ColorRole::Canvas),
            },
        ),
        (
            DepthLevel::RecededUtility,
            ResolvedMaterial {
                depth: DepthLevel::RecededUtility,
                intent: if frost_allowed {
                    MaterialIntent::Frosted
                } else {
                    MaterialIntent::Tonal
                },
                color: colors.get(ColorRole::UtilityReceded).with_alpha(opacity),
            },
        ),
        (
            DepthLevel::ActiveUtility,
            ResolvedMaterial {
                depth: DepthLevel::ActiveUtility,
                intent: if frost_allowed {
                    MaterialIntent::Frosted
                } else {
                    MaterialIntent::Tonal
                },
                color: colors
                    .get(ColorRole::UtilityActive)
                    .with_alpha(opacity.max(0.94)),
            },
        ),
        (
            DepthLevel::Attention,
            ResolvedMaterial {
                depth: DepthLevel::Attention,
                intent: if frost_allowed {
                    MaterialIntent::Frosted
                } else {
                    MaterialIntent::Tonal
                },
                color: colors.get(ColorRole::AttentionFill),
            },
        ),
    ];

    ResolvedVisual {
        appearance,
        metrics: Metrics::default()
            .with_user_padding(settings.window_padding, settings.terminal_padding),
        motion: MotionSettings::canonical(accessibility.reduce_motion),
        materials,
        ui_font,
        terminal_font,
        reduce_transparency: !frost_allowed,
        diagnostics,
        settings,
        colors,
        typography,
    }
}

pub fn canonical(
    appearance: ResolvedAppearance,
    accessibility: AccessibilitySignals,
) -> ResolvedVisual {
    let settings = UserUiSettings {
        appearance: match appearance {
            ResolvedAppearance::Dark => AppearancePreference::Dark,
            ResolvedAppearance::Light => AppearancePreference::Light,
        },
        ..UserUiSettings::default()
    };
    resolve(
        settings,
        appearance,
        accessibility,
        ConfigurationDiagnostics::default(),
    )
}
