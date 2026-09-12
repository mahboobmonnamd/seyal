import CoreGraphics

/// CoreText realization input for the Metal glyph atlas.
/// Portable theme/typography authority is Rust (#740). This is not a product palette.
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
