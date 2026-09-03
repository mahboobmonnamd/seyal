import AppKit

@MainActor
struct SeyalResolvedColors {
    private var values: [SeyalColorRole: SeyalRGBA]

    init(values: [SeyalColorRole: SeyalRGBA]) {
        self.values = values
    }

    subscript(_ role: SeyalColorRole) -> SeyalRGBA {
        values[role] ?? .srgb(0, 0, 0)
    }

    func ns(_ role: SeyalColorRole) -> NSColor {
        let color = self[role]
        return NSColor(
            srgbRed: color.red,
            green: color.green,
            blue: color.blue,
            alpha: color.alpha
        )
    }

    func cg(_ role: SeyalColorRole) -> CGColor {
        ns(role).cgColor
    }
}

@MainActor
struct SeyalResolvedTypography {
    private var fonts: [SeyalTypographyRole: NSFont]
    let specs: [SeyalTypographyRole: SeyalFontSpec]

    init(fonts: [SeyalTypographyRole: NSFont], specs: [SeyalTypographyRole: SeyalFontSpec]) {
        self.fonts = fonts
        self.specs = specs
    }

    subscript(_ role: SeyalTypographyRole) -> NSFont {
        fonts[role] ?? NSFont.systemFont(ofSize: 12)
    }
}

@MainActor
struct SeyalResolvedVisualConfiguration {
    let appearance: SeyalResolvedAppearance
    let settings: SeyalUserUISettings
    let colors: SeyalResolvedColors
    let typography: SeyalResolvedTypography
    let metrics: SeyalMetrics
    let motion: SeyalMotionSettings
    let materials: [SeyalDepthLevel: SeyalResolvedMaterial]
    let uiFont: SeyalResolvedFontSpec
    let terminalFont: SeyalResolvedFontSpec
    let reduceTransparency: Bool
    let diagnostics: SeyalConfigurationDiagnostics

    var nsAppearance: NSAppearance? {
        NSAppearance(named: appearance == .dark ? .darkAqua : .aqua)
    }

    func material(for depth: SeyalDepthLevel) -> SeyalResolvedMaterial {
        materials[depth] ?? SeyalResolvedMaterial(
            depth: depth,
            intent: depth == .truth ? .opaque : .tonal,
            color: colors[.container]
        )
    }

    func seamColor(_ role: SeyalSeamRole) -> NSColor {
        switch role {
        case .rest: colors.ns(.seamRest)
        case .hover: colors.ns(.seamHover)
        case .focus: colors.ns(.seamFocus)
        case .running: colors.ns(.seamRunning)
        case .attention: colors.ns(.seamAttention)
        }
    }
}

enum SeyalThemeResolver {
    @MainActor
    static func resolve(
        settings: SeyalUserUISettings,
        platformAppearance: SeyalResolvedAppearance,
        accessibility: SeyalAccessibilitySignals = .none,
        diagnostics: SeyalConfigurationDiagnostics = SeyalConfigurationDiagnostics()
    ) -> SeyalResolvedVisualConfiguration {
        var settings = settings
        settings.clampToBounds()

        let appearance: SeyalResolvedAppearance
        switch settings.appearance {
        case .system: appearance = platformAppearance
        case .light: appearance = .light
        case .dark: appearance = .dark
        }

        let colors = SeyalResolvedColors(
            values: Dictionary(
                uniqueKeysWithValues: SeyalColorRole.allCases.map { role in
                    (
                        role,
                        SeyalProductPalette.color(
                            role,
                            appearance: appearance,
                            increaseContrast: accessibility.increaseContrast
                        )
                    )
                }
            )
        )

        let uiFont = SeyalResolvedFontSpec(
            family: settings.uiFontFamily,
            fallbacks: settings.uiFontFallbacks,
            pointSize: settings.uiFontSize
        )
        let terminalFont = SeyalResolvedFontSpec(
            family: settings.terminalFontFamily,
            fallbacks: settings.terminalFontFallbacks,
            pointSize: settings.terminalFontSize
        )
        let specs = SeyalTypographyCatalog.specs(ui: uiFont, terminal: terminalFont)
        let fonts = Dictionary(
            uniqueKeysWithValues: specs.map { role, spec in
                (role, SeyalUIFontResolver.font(for: spec, isTerminal: role == .terminal || role == .composer))
            }
        )

        let frostAllowed = !settings.reducedMaterial && !accessibility.reduceTransparency
        let opacity = frostAllowed ? settings.utilityOpacity : 1
        let materials: [SeyalDepthLevel: SeyalResolvedMaterial] = [
            .truth: .init(depth: .truth, intent: .opaque, color: colors[.canvas]),
            .recededUtility: .init(
                depth: .recededUtility,
                intent: frostAllowed ? .frosted : .tonal,
                color: colors[.utilityReceded].withAlpha(Double(opacity))
            ),
            .activeUtility: .init(
                depth: .activeUtility,
                intent: frostAllowed ? .frosted : .tonal,
                color: colors[.utilityActive].withAlpha(Double(max(opacity, 0.94)))
            ),
            .attention: .init(
                depth: .attention,
                intent: frostAllowed ? .frosted : .tonal,
                color: colors[.attentionFill]
            ),
        ]

        return SeyalResolvedVisualConfiguration(
            appearance: appearance,
            settings: settings,
            colors: colors,
            typography: SeyalResolvedTypography(fonts: fonts, specs: specs),
            metrics: SeyalMetrics.canonical.withUserPadding(
                window: settings.windowPadding,
                terminal: settings.terminalPadding
            ),
            motion: SeyalMotionSettings.canonical(reducedMotion: accessibility.reduceMotion),
            materials: materials,
            uiFont: uiFont,
            terminalFont: terminalFont,
            reduceTransparency: !frostAllowed,
            diagnostics: diagnostics
        )
    }

    @MainActor
    static func canonical(
        _ appearance: SeyalResolvedAppearance,
        accessibility: SeyalAccessibilitySignals = .none
    ) -> SeyalResolvedVisualConfiguration {
        var settings = SeyalUserUISettings.default
        settings.appearance = appearance == .dark ? .dark : .light
        return resolve(
            settings: settings,
            platformAppearance: appearance,
            accessibility: accessibility
        )
    }
}

enum SeyalUIFontResolver {
    @MainActor
    static func font(for spec: SeyalFontSpec, isTerminal: Bool) -> NSFont {
        let weight = nsWeight(spec.weight)
        let candidates = ([spec.family] + spec.fallbacks)
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }

        for family in candidates {
            if let named = NSFont(name: family, size: spec.size) {
                return named
            }
            let manager = NSFontManager.shared
            if let converted = manager.font(withFamily: family, traits: [], weight: managerWeight(spec.weight), size: spec.size) {
                return converted
            }
        }

        if isTerminal {
            // Only the unresolved-family fallback may use the system mono face.
            return NSFont.monospacedSystemFont(ofSize: spec.size, weight: weight)
        }
        return NSFont.systemFont(ofSize: spec.size, weight: weight)
    }

    private static func nsWeight(_ weight: SeyalFontWeight) -> NSFont.Weight {
        switch weight {
        case .regular: .regular
        case .medium: .medium
        case .semibold: .semibold
        case .bold: .bold
        }
    }

    private static func managerWeight(_ weight: SeyalFontWeight) -> Int {
        switch weight {
        case .regular: 5
        case .medium: 6
        case .semibold: 8
        case .bold: 9
        }
    }
}
