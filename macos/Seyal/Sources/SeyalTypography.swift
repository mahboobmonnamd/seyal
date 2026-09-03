import CoreGraphics
import Foundation

enum SeyalTypographyRole: String, Equatable, Sendable, CaseIterable {
    case windowTitle
    case sectionLabel
    case uiBody
    case uiSecondary
    case metadata
    case tab
    case sidebarRow
    case inspectorHeading
    case action
    case composer
    case terminal
}

enum SeyalFontWeight: String, Equatable, Sendable {
    case regular
    case medium
    case semibold
    case bold
}

struct SeyalFontSpec: Equatable, Sendable {
    var family: String
    var fallbacks: [String]
    var size: CGFloat
    var weight: SeyalFontWeight
    var lineHeight: CGFloat
    var tracking: CGFloat

    static func ui(
        family: String,
        fallbacks: [String],
        size: CGFloat,
        weight: SeyalFontWeight,
        lineHeight: CGFloat,
        tracking: CGFloat = 0
    ) -> SeyalFontSpec {
        SeyalFontSpec(
            family: family,
            fallbacks: fallbacks,
            size: size,
            weight: weight,
            lineHeight: lineHeight,
            tracking: tracking
        )
    }
}

struct SeyalResolvedFontSpec: Equatable, Sendable {
    var family: String
    var fallbacks: [String]
    var pointSize: CGFloat

    static let canonicalUI = SeyalResolvedFontSpec(
        family: "",
        fallbacks: ["SF Pro Text", "Helvetica Neue"],
        pointSize: 12
    )

    static let canonicalTerminal = SeyalResolvedFontSpec(
        family: "Menlo",
        fallbacks: ["SF Mono", "Menlo", "Courier"],
        pointSize: 14
    )
}

enum SeyalTypographyCatalog {
    static func specs(
        ui: SeyalResolvedFontSpec,
        terminal: SeyalResolvedFontSpec
    ) -> [SeyalTypographyRole: SeyalFontSpec] {
        let uiFamily = ui.family
        let uiFallbacks = ui.fallbacks
        let body = ui.pointSize
        let small = max(9, body - 2)
        var specs: [SeyalTypographyRole: SeyalFontSpec] = [:]
        specs[.windowTitle] = .ui(
            family: uiFamily, fallbacks: uiFallbacks, size: body, weight: .semibold,
            lineHeight: body + 4)
        specs[.sectionLabel] = .ui(
            family: uiFamily, fallbacks: uiFallbacks, size: small, weight: .semibold,
            lineHeight: small + 3, tracking: 0.4)
        specs[.uiBody] = .ui(
            family: uiFamily, fallbacks: uiFallbacks, size: body, weight: .regular,
            lineHeight: body + 4)
        specs[.uiSecondary] = .ui(
            family: uiFamily, fallbacks: uiFallbacks, size: body, weight: .regular,
            lineHeight: body + 4)
        specs[.metadata] = .ui(
            family: uiFamily, fallbacks: uiFallbacks, size: small, weight: .regular,
            lineHeight: small + 3)
        specs[.tab] = .ui(
            family: uiFamily, fallbacks: uiFallbacks, size: body, weight: .medium,
            lineHeight: body + 4)
        specs[.sidebarRow] = .ui(
            family: uiFamily, fallbacks: uiFallbacks, size: body, weight: .regular,
            lineHeight: body + 4)
        specs[.inspectorHeading] = .ui(
            family: uiFamily, fallbacks: uiFallbacks, size: small, weight: .semibold,
            lineHeight: small + 3, tracking: 0.3)
        specs[.action] = .ui(
            family: uiFamily, fallbacks: uiFallbacks, size: body, weight: .medium,
            lineHeight: body + 4)
        specs[.composer] = .ui(
            family: terminal.family, fallbacks: terminal.fallbacks, size: body,
            weight: .semibold, lineHeight: body + 6)
        specs[.terminal] = .ui(
            family: terminal.family, fallbacks: terminal.fallbacks, size: terminal.pointSize,
            weight: .regular, lineHeight: terminal.pointSize + 5)
        return specs
    }
}
