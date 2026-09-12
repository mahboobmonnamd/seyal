import Foundation

/// Native CoreText font request. Product theme/config authority stays in Rust (#740).
struct SeyalResolvedFontSpec: Equatable, Sendable {
    var family: String
    var fallbacks: [String]
    var pointSize: CGFloat

    static let canonicalTerminal = SeyalResolvedFontSpec(
        family: "Menlo",
        fallbacks: ["SF Mono", "Menlo", "Courier"],
        pointSize: 14
    )
}
