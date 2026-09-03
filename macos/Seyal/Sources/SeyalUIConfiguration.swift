import Foundation

enum SeyalUIConfiguration {
    static let supportedKeys: Set<String> = [
        "ui.appearance",
        "ui.reduced-material",
        "ui.utility-opacity",
        "ui.window-padding",
        "ui.font.family",
        "ui.font.size",
        "ui.font.fallbacks",
        "terminal.padding",
        "terminal.font.family",
        "terminal.font.size",
        "terminal.font.fallbacks",
    ]

    struct LoadResult: Equatable, Sendable {
        var settings: SeyalUserUISettings
        var diagnostics: SeyalConfigurationDiagnostics
        var source: String
    }

    static func load(
        tomlText: String?,
        overlay: (any SeyalColdConfigurationOverlay)? = nil,
        environment: [String: String] = [:],
        defaultSettings: SeyalUserUISettings = .default
    ) -> LoadResult {
        var diagnostics = SeyalConfigurationDiagnostics()
        var settings = defaultSettings
        var source = "defaults"

        if let tomlText {
            switch SeyalTOMLParser.parse(tomlText) {
            case let .success(table):
                apply(table: table, to: &settings, diagnostics: &diagnostics)
                source = "toml"
            case let .failure(error):
                diagnostics.warnings.append("TOML ignored: \(error)")
                diagnostics.usedFullDefaultFallback = true
                settings = defaultSettings
                source = "defaults"
            }
        }

        applyEnvironment(environment, to: &settings, diagnostics: &diagnostics)

        if let overlay {
            do {
                settings = try overlay.configPatch().applying(to: settings)
                source = source == "defaults" ? "overlay" : "\(source)+overlay"
            } catch {
                diagnostics.warnings.append("Lua overlay ignored: \(error.localizedDescription)")
            }
        }

        settings.clampToBounds()
        return LoadResult(settings: settings, diagnostics: diagnostics, source: source)
    }

    static func loadFromDisk(
        environment: [String: String] = ProcessInfo.processInfo.environment,
        overlay: (any SeyalColdConfigurationOverlay)? = nil
    ) -> LoadResult {
        let explicit = environment["SEYAL_CONFIG"]
        let home = FileManager.default.homeDirectoryForCurrentUser
        let defaultURL = home.appendingPathComponent(".config/seyal/config.toml")
        let url = explicit.map(URL.init(fileURLWithPath:)) ?? defaultURL
        let text = try? String(contentsOf: url, encoding: .utf8)
        return load(tomlText: text, overlay: overlay, environment: environment)
    }

    private static func apply(
        table: [String: SeyalTOMLValue],
        to settings: inout SeyalUserUISettings,
        diagnostics: inout SeyalConfigurationDiagnostics
    ) {
        let ui = table["ui"]?.table ?? [:]
        let uiFont = ui["font"]?.table ?? [:]
        let terminal = table["terminal"]?.table ?? [:]
        let terminalFont = terminal["font"]?.table ?? [:]

        warnUnknown(prefix: "ui", table: ui, known: ["appearance", "reduced-material", "utility-opacity", "window-padding", "font"], diagnostics: &diagnostics)
        warnUnknown(prefix: "ui.font", table: uiFont, known: ["family", "size", "fallbacks"], diagnostics: &diagnostics)
        warnUnknown(prefix: "terminal", table: terminal, known: ["padding", "font"], diagnostics: &diagnostics)
        warnUnknown(prefix: "terminal.font", table: terminalFont, known: ["family", "size", "fallbacks"], diagnostics: &diagnostics)

        if let value = ui["appearance"] {
            if let raw = value.string, let parsed = SeyalAppearancePreference(rawValue: raw) {
                settings.appearance = parsed
            } else {
                diagnostics.warnings.append("ui.appearance ignored; expected system|light|dark")
            }
        }
        assignBool(ui["reduced-material"], into: &settings.reducedMaterial, key: "ui.reduced-material", diagnostics: &diagnostics)
        assignNumber(ui["utility-opacity"], range: SeyalUserUISettings.opacityRange, into: &settings.utilityOpacity, key: "ui.utility-opacity", diagnostics: &diagnostics)
        assignNumber(ui["window-padding"], range: SeyalUserUISettings.paddingRange, into: &settings.windowPadding, key: "ui.window-padding", diagnostics: &diagnostics)
        assignString(uiFont["family"], into: &settings.uiFontFamily, key: "ui.font.family", diagnostics: &diagnostics)
        assignNumber(uiFont["size"], range: SeyalUserUISettings.uiFontSizeRange, into: &settings.uiFontSize, key: "ui.font.size", diagnostics: &diagnostics)
        assignStringArray(uiFont["fallbacks"], into: &settings.uiFontFallbacks, key: "ui.font.fallbacks", diagnostics: &diagnostics)
        assignNumber(terminal["padding"], range: SeyalUserUISettings.paddingRange, into: &settings.terminalPadding, key: "terminal.padding", diagnostics: &diagnostics)
        assignString(terminalFont["family"], into: &settings.terminalFontFamily, key: "terminal.font.family", diagnostics: &diagnostics)
        assignNumber(terminalFont["size"], range: SeyalUserUISettings.terminalFontSizeRange, into: &settings.terminalFontSize, key: "terminal.font.size", diagnostics: &diagnostics)
        assignStringArray(terminalFont["fallbacks"], into: &settings.terminalFontFallbacks, key: "terminal.font.fallbacks", diagnostics: &diagnostics)
    }

    private static func applyEnvironment(
        _ environment: [String: String],
        to settings: inout SeyalUserUISettings,
        diagnostics: inout SeyalConfigurationDiagnostics
    ) {
        if let raw = environment["SEYAL_UI_APPEARANCE"] {
            if let parsed = SeyalAppearancePreference(rawValue: raw) {
                settings.appearance = parsed
            } else {
                diagnostics.warnings.append("SEYAL_UI_APPEARANCE ignored; expected system|light|dark")
            }
        }
        if environment["SEYAL_UI_REDUCED_MATERIAL"] == "1" {
            settings.reducedMaterial = true
        }
    }

    private static func warnUnknown(
        prefix: String,
        table: [String: SeyalTOMLValue],
        known: Set<String>,
        diagnostics: inout SeyalConfigurationDiagnostics
    ) {
        for key in table.keys where !known.contains(key) {
            diagnostics.warnings.append("unknown key \(prefix).\(key) ignored")
        }
    }

    private static func assignBool(
        _ value: SeyalTOMLValue?,
        into target: inout Bool,
        key: String,
        diagnostics: inout SeyalConfigurationDiagnostics
    ) {
        guard let value else { return }
        if let parsed = value.bool {
            target = parsed
        } else {
            diagnostics.warnings.append("\(key) ignored; expected boolean")
        }
    }

    private static func assignString(
        _ value: SeyalTOMLValue?,
        into target: inout String,
        key: String,
        diagnostics: inout SeyalConfigurationDiagnostics
    ) {
        guard let value else { return }
        if let parsed = value.string {
            target = parsed
        } else {
            diagnostics.warnings.append("\(key) ignored; expected string")
        }
    }

    private static func assignStringArray(
        _ value: SeyalTOMLValue?,
        into target: inout [String],
        key: String,
        diagnostics: inout SeyalConfigurationDiagnostics
    ) {
        guard let value else { return }
        if let parsed = value.stringArray {
            target = SeyalUserUISettings.sanitizeFamilies(parsed)
        } else {
            diagnostics.warnings.append("\(key) ignored; expected array of strings")
        }
    }

    private static func assignNumber(
        _ value: SeyalTOMLValue?,
        range: ClosedRange<CGFloat>,
        into target: inout CGFloat,
        key: String,
        diagnostics: inout SeyalConfigurationDiagnostics
    ) {
        guard let value else { return }
        guard let parsed = value.number else {
            diagnostics.warnings.append("\(key) ignored; expected number")
            return
        }
        let number = CGFloat(parsed)
        if range.contains(number) {
            target = number
        } else {
            target = SeyalUserUISettings.clamp(number, range)
            diagnostics.warnings.append("\(key) clamped to \(target)")
        }
    }
}
