import Foundation

/// Adaptive Depth levels. These describe presentation intent, not a platform
/// blur API.
enum SeyalDepthLevel: String, Equatable, Sendable, CaseIterable {
    /// Terminal/TUI/Block output truth. Opaque, no frost.
    case truth
    /// Persistent but currently secondary utility.
    case recededUtility
    /// Utility the user is actively operating.
    case activeUtility
    /// Temporary operational attention.
    case attention
}

/// How a depth level should be realized. The native host maps this to AppKit.
enum SeyalMaterialIntent: String, Equatable, Sendable {
    case opaque
    case tonal
    case frosted
}

struct SeyalResolvedMaterial: Equatable, Sendable {
    var depth: SeyalDepthLevel
    var intent: SeyalMaterialIntent
    var color: SeyalRGBA
}

enum SeyalSeamRole: String, Equatable, Sendable {
    case rest
    case hover
    case focus
    case running
    case attention
}
