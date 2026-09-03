import CoreGraphics
import Foundation

/// User-facing UI overrides. This is not the product token table.
struct SeyalUserUISettings: Equatable, Sendable {
    var appearance: SeyalAppearancePreference = .system
    var uiFontFamily: String = ""
    var uiFontSize: CGFloat = 12
    var uiFontFallbacks: [String] = ["SF Pro Text", "Helvetica Neue"]
    var terminalFontFamily: String = "Menlo"
    var terminalFontSize: CGFloat = 14
    var terminalFontFallbacks: [String] = ["SF Mono", "Menlo", "Courier"]
    var windowPadding: CGFloat = 0
    var terminalPadding: CGFloat = 8
    var utilityOpacity: CGFloat = 1
    var reducedMaterial: Bool = false

    static let `default` = SeyalUserUISettings()

    static let uiFontSizeRange: ClosedRange<CGFloat> = 10...18
    static let terminalFontSizeRange: ClosedRange<CGFloat> = 9...22
    static let paddingRange: ClosedRange<CGFloat> = 0...24
    static let opacityRange: ClosedRange<CGFloat> = 0.85...1.0

    mutating func clampToBounds() {
        uiFontSize = Self.clamp(uiFontSize, Self.uiFontSizeRange)
        terminalFontSize = Self.clamp(terminalFontSize, Self.terminalFontSizeRange)
        windowPadding = Self.clamp(windowPadding, Self.paddingRange)
        terminalPadding = Self.clamp(terminalPadding, Self.paddingRange)
        utilityOpacity = Self.clamp(utilityOpacity, Self.opacityRange)
        uiFontFallbacks = Self.sanitizeFamilies(uiFontFallbacks)
        terminalFontFallbacks = Self.sanitizeFamilies(terminalFontFallbacks)
        if terminalFontFamily.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            terminalFontFamily = Self.default.terminalFontFamily
        }
    }

    static func clamp(_ value: CGFloat, _ range: ClosedRange<CGFloat>) -> CGFloat {
        min(max(value, range.lowerBound), range.upperBound)
    }

    static func sanitizeFamilies(_ families: [String]) -> [String] {
        families
            .map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }
            .filter { !$0.isEmpty }
    }
}

/// Typed Lua/config overlay. A future Lua runtime may only produce this patch
/// at a cold load/reload boundary.
struct SeyalConfigPatch: Equatable, Sendable {
    var appearance: SeyalAppearancePreference?
    var uiFontFamily: String?
    var uiFontSize: CGFloat?
    var uiFontFallbacks: [String]?
    var terminalFontFamily: String?
    var terminalFontSize: CGFloat?
    var terminalFontFallbacks: [String]?
    var windowPadding: CGFloat?
    var terminalPadding: CGFloat?
    var utilityOpacity: CGFloat?
    var reducedMaterial: Bool?

    static let empty = SeyalConfigPatch()

    func applying(to settings: SeyalUserUISettings) -> SeyalUserUISettings {
        var next = settings
        if let appearance { next.appearance = appearance }
        if let uiFontFamily { next.uiFontFamily = uiFontFamily }
        if let uiFontSize { next.uiFontSize = uiFontSize }
        if let uiFontFallbacks { next.uiFontFallbacks = uiFontFallbacks }
        if let terminalFontFamily { next.terminalFontFamily = terminalFontFamily }
        if let terminalFontSize { next.terminalFontSize = terminalFontSize }
        if let terminalFontFallbacks { next.terminalFontFallbacks = terminalFontFallbacks }
        if let windowPadding { next.windowPadding = windowPadding }
        if let terminalPadding { next.terminalPadding = terminalPadding }
        if let utilityOpacity { next.utilityOpacity = utilityOpacity }
        if let reducedMaterial { next.reducedMaterial = reducedMaterial }
        next.clampToBounds()
        return next
    }
}

/// Cold overlay producer. Must not be invoked from terminal hot paths.
protocol SeyalColdConfigurationOverlay: Sendable {
    func configPatch() throws -> SeyalConfigPatch
}

struct SeyalLuaConfigurationBoundary {
    static let acceptedInput = "SeyalConfigPatch"
    static let executionDomain = "cold load/reload only"
    static let runtimeStatus = "deferred; no Lua VM in this milestone"

    static let forbiddenDomains: [String] = [
        "keystrokes",
        "PTY input/output",
        "VT parsing",
        "terminal-grid updates",
        "damage tracking",
        "Metal rendering",
        "per-frame presentation",
        "direct NSView mutation",
    ]
}

struct SeyalConfigurationDiagnostics: Equatable, Sendable {
    var warnings: [String] = []
    var usedFullDefaultFallback = false

    var isClean: Bool { warnings.isEmpty && !usedFullDefaultFallback }
}
