import CoreGraphics
import Foundation

/// Device-independent sRGB colour. Token tables store this type so palettes
/// stay Sendable and independent of AppKit actor isolation.
struct SeyalRGBA: Equatable, Sendable {
    var red: Double
    var green: Double
    var blue: Double
    var alpha: Double

    static func srgb(_ red: Double, _ green: Double, _ blue: Double, alpha: Double = 1) -> Self {
        Self(red: red / 255, green: green / 255, blue: blue / 255, alpha: alpha)
    }

    var luminance: Double {
        0.2126 * red + 0.7152 * green + 0.0722 * blue
    }

    func withAlpha(_ alpha: Double) -> SeyalRGBA {
        var copy = self
        copy.alpha = min(max(alpha, 0), 1)
        return copy
    }
}

enum SeyalColorRole: String, Equatable, Sendable, CaseIterable {
    case canvas
    case container
    case utilityReceded
    case utilityActive
    case utilityElevated
    case overlay
    case attentionFill
    case textPrimary
    case textSecondary
    case textMuted
    case textAttention
    case seamRest
    case seamHover
    case seamFocus
    case seamRunning
    case seamAttention
    case focus
    case selection
    case success
    case warning
    case danger
    case information
    case agentActivity
    case remoteDegraded
}

enum SeyalAppearancePreference: String, Equatable, Sendable {
    case system
    case light
    case dark
}

enum SeyalResolvedAppearance: String, Equatable, Sendable {
    case light
    case dark
}
