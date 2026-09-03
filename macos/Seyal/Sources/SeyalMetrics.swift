import CoreGraphics
import Foundation

/// Canonical geometry. User configuration may override only the documented
/// padding fields; the rest is product design authority.
struct SeyalMetrics: Equatable, Sendable {
    var base: CGFloat = 4
    var xs: CGFloat = 4
    var sm: CGFloat = 8
    var md: CGFloat = 12
    var lg: CGFloat = 16

    var windowPadding: CGFloat = 0
    var contentPaddingHorizontal: CGFloat = 12
    var contentPaddingVertical: CGFloat = 10
    var sidebarPadding: CGFloat = 10
    var inspectorPadding: CGFloat = 10
    var tabSpacing: CGFloat = 8
    var blockSeamSpacing: CGFloat = 8
    var composerInsetHorizontal: CGFloat = 12
    var composerInsetVertical: CGFloat = 8
    var controlSpacing: CGFloat = 8
    var paneSeparatorThickness: CGFloat = 1

    var utilityRailWidth: CGFloat = 36
    var leftContextWidth: CGFloat = 220
    var leftContextMinWidth: CGFloat = 180
    var inspectorWidth: CGFloat = 248
    var inspectorMinWidth: CGFloat = 200
    var inspectorRailWidth: CGFloat = 36
    var topChromeHeight: CGFloat = 48
    var composerMinHeight: CGFloat = 52
    var composerMaxHeight: CGFloat = 116
    var minInteractiveSize: CGFloat = 28
    var tabMinWidth: CGFloat = 118
    var tabMaxWidth: CGFloat = 190

    /// Blocks are not cards. Utility/composer may use a modest radius.
    var blockCornerRadius: CGFloat = 0
    var paneCornerRadius: CGFloat = 0
    var composerCornerRadius: CGFloat = 6
    var overlayCornerRadius: CGFloat = 8
    var seamWidth: CGFloat = 1
    var terminalPadding: CGFloat = 8

    static let canonical = SeyalMetrics()

    func withUserPadding(window: CGFloat, terminal: CGFloat) -> SeyalMetrics {
        var copy = self
        copy.windowPadding = window
        copy.terminalPadding = terminal
        return copy
    }

    static func validate(_ metrics: SeyalMetrics) -> Bool {
        metrics.leftContextWidth >= metrics.leftContextMinWidth
            && metrics.inspectorWidth >= metrics.inspectorMinWidth
            && metrics.composerMinHeight <= metrics.composerMaxHeight
            && metrics.minInteractiveSize >= 20
            && metrics.seamWidth > 0
            && metrics.seamWidth <= 2
            && metrics.blockCornerRadius == 0
            && metrics.paneCornerRadius == 0
            && metrics.base == metrics.xs
    }
}

struct SeyalMotionSettings: Equatable, Sendable {
    var allowsMotion: Bool
    var focusDuration: TimeInterval
    var overlayDuration: TimeInterval

    static func canonical(reducedMotion: Bool) -> SeyalMotionSettings {
        SeyalMotionSettings(
            allowsMotion: !reducedMotion,
            focusDuration: reducedMotion ? 0 : 0.12,
            overlayDuration: reducedMotion ? 0 : 0.16
        )
    }
}

struct SeyalAccessibilitySignals: Equatable, Sendable {
    var reduceTransparency: Bool
    var reduceMotion: Bool
    var increaseContrast: Bool

    static let none = SeyalAccessibilitySignals(
        reduceTransparency: false,
        reduceMotion: false,
        increaseContrast: false
    )
}
