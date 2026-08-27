import CoreGraphics
import CoreText
import Foundation
import Metal

struct TerminalFontMetrics: Equatable {
    let cellWidth: Int
    let cellHeight: Int
    let ascent: CGFloat
    let descent: CGFloat
}

struct GlyphAtlasEntry {
    let uvRect: SIMD4<Float>
    let slice: UInt32
}

struct GlyphAtlasStats: Equatable {
    var hits: UInt64 = 0
    var misses: UInt64 = 0
    var uploads: UInt64 = 0
    var uploadedBytes: UInt64 = 0
    var resets: UInt64 = 0
}

enum GlyphAtlasError: Error {
    case unavailableFont
    case unavailableTexture
    case capacityExceeded
    case rasterizationFailed
}

private struct GlyphKey: Hashable {
    let fontName: String
    let glyph: UInt16
    let pixelSizeBits: UInt64
    let bold: Bool
}

private struct ResolvedGlyph {
    let font: CTFont
    let glyph: CGGlyph
    let fontName: String
}

private struct AtlasCursor {
    var x = 1
    var y = 1
    var rowHeight = 0
}

final class TerminalFontResolver {
    private let pointSize: CGFloat
    private let regular: CTFont
    private let bold: CTFont

    init(pointSize: CGFloat = 14) {
        self.pointSize = pointSize
        regular = CTFontCreateWithName("Menlo" as CFString, pointSize, nil)
        bold = CTFontCreateWithName("Menlo-Bold" as CFString, pointSize, nil)
    }

    func metrics(backingScale: CGFloat) -> TerminalFontMetrics {
        let scale = max(backingScale, 1)
        let pixelFont = CTFontCreateCopyWithAttributes(regular, pointSize * scale, nil, nil)
        var glyph = glyphForBMPScalar(77, font: pixelFont) ?? 0
        var advance = CGSize.zero
        _ = CTFontGetAdvancesForGlyphs(pixelFont, .horizontal, &glyph, &advance, 1)
        let width = max(1, Int(ceil(advance.width)))
        let ascent = CTFontGetAscent(pixelFont)
        let descent = CTFontGetDescent(pixelFont)
        let leading = CTFontGetLeading(pixelFont)
        let height = max(1, Int(ceil(ascent + descent + leading)))
        return TerminalFontMetrics(
            cellWidth: width,
            cellHeight: height,
            ascent: ascent,
            descent: descent
        )
    }

    fileprivate func resolve(
        scalar: UInt32,
        bold requestedBold: Bool,
        backingScale: CGFloat
    ) -> ResolvedGlyph? {
        guard let unicodeScalar = UnicodeScalar(scalar) else {
            return resolveReplacement(bold: requestedBold, backingScale: backingScale)
        }
        let text = String(unicodeScalar)
        let base = requestedBold ? bold : regular
        let utf16Length = (text as NSString).length
        let fallback = CTFontCreateForString(
            base,
            text as CFString,
            CFRange(location: 0, length: utf16Length)
        )
        let scale = max(backingScale, 1)
        let pixelFont = CTFontCreateCopyWithAttributes(
            fallback,
            CTFontGetSize(fallback) * scale,
            nil,
            nil
        )

        guard scalar <= 0xffff,
              let glyph = glyphForBMPScalar(scalar, font: pixelFont),
              glyph != 0
        else {
            return resolveReplacement(bold: requestedBold, backingScale: backingScale)
        }

        return ResolvedGlyph(
            font: pixelFont,
            glyph: glyph,
            fontName: CTFontCopyPostScriptName(pixelFont) as String
        )
    }

    private func resolveReplacement(
        bold requestedBold: Bool,
        backingScale: CGFloat
    ) -> ResolvedGlyph? {
        let replacement: UInt32 = 0xfffd
        let base = requestedBold ? bold : regular
        let text = "\u{fffd}"
        let fallback = CTFontCreateForString(
            base,
            text as CFString,
            CFRange(location: 0, length: 1)
        )
        let scale = max(backingScale, 1)
        let pixelFont = CTFontCreateCopyWithAttributes(
            fallback,
            CTFontGetSize(fallback) * scale,
            nil,
            nil
        )
        guard let glyph = glyphForBMPScalar(replacement, font: pixelFont), glyph != 0 else {
            return nil
        }
        return ResolvedGlyph(
            font: pixelFont,
            glyph: glyph,
            fontName: CTFontCopyPostScriptName(pixelFont) as String
        )
    }

    private func glyphForBMPScalar(_ scalar: UInt32, font: CTFont) -> CGGlyph? {
        guard scalar <= 0xffff else { return nil }
        var character = UniChar(scalar)
        var glyph: CGGlyph = 0
        guard CTFontGetGlyphsForCharacters(font, &character, &glyph, 1) else { return nil }
        return glyph
    }
}

final class GlyphAtlas {
    static let width = 2048
    static let height = 2048
    static let sliceCount = 4
    static let bytesPerPixel = 1
    static let budgetBytes = width * height * sliceCount * bytesPerPixel

    private let device: MTLDevice
    private let fontResolver: TerminalFontResolver
    private var textureStorage: MTLTexture?
    private var entries: [GlyphKey: GlyphAtlasEntry] = [:]
    private var cursors = Array(repeating: AtlasCursor(), count: sliceCount)
    private(set) var stats = GlyphAtlasStats()

    init(device: MTLDevice, fontResolver: TerminalFontResolver = TerminalFontResolver()) {
        self.device = device
        self.fontResolver = fontResolver
    }

    var texture: MTLTexture? {
        textureStorage
    }

    var entryCount: Int {
        entries.count
    }

    var estimatedResidentBytes: Int {
        textureStorage == nil ? 0 : Self.budgetBytes
    }

    func metrics(backingScale: CGFloat) -> TerminalFontMetrics {
        fontResolver.metrics(backingScale: backingScale)
    }

    @discardableResult
    func ensureTextureForRendering() throws -> MTLTexture {
        try ensureTexture()
    }

    func lookup(
        scalar: UInt32,
        bold: Bool,
        backingScale: CGFloat,
        cellMetrics: TerminalFontMetrics
    ) throws -> GlyphAtlasEntry {
        guard let resolved = fontResolver.resolve(
            scalar: scalar,
            bold: bold,
            backingScale: backingScale
        ) else {
            throw GlyphAtlasError.unavailableFont
        }
        let key = GlyphKey(
            fontName: resolved.fontName,
            glyph: resolved.glyph,
            pixelSizeBits: Double(CTFontGetSize(resolved.font)).bitPattern,
            bold: bold
        )
        if let entry = entries[key] {
            stats.hits &+= 1
            return entry
        }

        stats.misses &+= 1
        let bitmap = try rasterize(
            resolved: resolved,
            width: cellMetrics.cellWidth,
            height: cellMetrics.cellHeight
        )
        guard let allocation = allocate(width: bitmap.width, height: bitmap.height) else {
            throw GlyphAtlasError.capacityExceeded
        }
        let texture = try ensureTexture()
        bitmap.bytes.withUnsafeBytes { rawBuffer in
            guard let baseAddress = rawBuffer.baseAddress else { return }
            texture.replace(
                region: MTLRegionMake2D(allocation.x, allocation.y, bitmap.width, bitmap.height),
                mipmapLevel: 0,
                slice: allocation.slice,
                withBytes: baseAddress,
                bytesPerRow: bitmap.width,
                bytesPerImage: bitmap.width * bitmap.height
            )
        }

        let atlasWidth = Float(Self.width)
        let atlasHeight = Float(Self.height)
        let entry = GlyphAtlasEntry(
            uvRect: SIMD4<Float>(
                Float(allocation.x) / atlasWidth,
                Float(allocation.y) / atlasHeight,
                Float(allocation.x + bitmap.width) / atlasWidth,
                Float(allocation.y + bitmap.height) / atlasHeight
            ),
            slice: UInt32(allocation.slice)
        )
        entries[key] = entry
        stats.uploads &+= 1
        stats.uploadedBytes &+= UInt64(bitmap.width * bitmap.height)
        return entry
    }

    /// Reclaim the finite atlas. Callers must prove no submitted command buffer
    /// can still reference the old texture before invoking this method.
    func resetWhenGPUIdle() {
        textureStorage = nil
        entries.removeAll(keepingCapacity: true)
        cursors = Array(repeating: AtlasCursor(), count: Self.sliceCount)
        stats.resets &+= 1
    }

    func releaseResourcesWhenGPUIdle() {
        resetWhenGPUIdle()
        entries.removeAll(keepingCapacity: false)
    }

    private func ensureTexture() throws -> MTLTexture {
        if let textureStorage {
            return textureStorage
        }
        let descriptor = MTLTextureDescriptor()
        descriptor.textureType = .type2DArray
        descriptor.pixelFormat = .r8Unorm
        descriptor.width = Self.width
        descriptor.height = Self.height
        descriptor.arrayLength = Self.sliceCount
        descriptor.mipmapLevelCount = 1
        descriptor.usage = [.shaderRead]
        descriptor.storageMode = .shared
        guard let texture = device.makeTexture(descriptor: descriptor) else {
            throw GlyphAtlasError.unavailableTexture
        }
        texture.label = "Seyal Glyph Atlas"
        textureStorage = texture
        return texture
    }

    private func allocate(width: Int, height: Int) -> (slice: Int, x: Int, y: Int)? {
        guard width > 0, height > 0,
              width + 2 <= Self.width,
              height + 2 <= Self.height
        else {
            return nil
        }

        for slice in 0..<Self.sliceCount {
            var cursor = cursors[slice]
            if cursor.x + width + 1 > Self.width {
                cursor.x = 1
                cursor.y += cursor.rowHeight + 1
                cursor.rowHeight = 0
            }
            guard cursor.y + height + 1 <= Self.height else {
                continue
            }
            let allocation = (slice: slice, x: cursor.x, y: cursor.y)
            cursor.x += width + 1
            cursor.rowHeight = max(cursor.rowHeight, height)
            cursors[slice] = cursor
            return allocation
        }
        return nil
    }

    private func rasterize(
        resolved: ResolvedGlyph,
        width: Int,
        height: Int
    ) throws -> (bytes: [UInt8], width: Int, height: Int) {
        guard width > 0, height > 0 else {
            throw GlyphAtlasError.rasterizationFailed
        }
        var bytes = [UInt8](repeating: 0, count: width * height)
        let colorSpace = CGColorSpaceCreateDeviceGray()
        let madeContext = bytes.withUnsafeMutableBytes { rawBuffer -> Bool in
            guard let baseAddress = rawBuffer.baseAddress,
                  let context = CGContext(
                      data: baseAddress,
                      width: width,
                      height: height,
                      bitsPerComponent: 8,
                      bytesPerRow: width,
                      space: colorSpace,
                      bitmapInfo: CGImageAlphaInfo.none.rawValue
                  )
            else {
                return false
            }
            context.setFillColor(gray: 1, alpha: 1)
            context.setShouldAntialias(true)
            context.setAllowsAntialiasing(true)
            context.textMatrix = .identity

            var glyph = resolved.glyph
            let bounds = CTFontGetBoundingRectsForGlyphs(resolved.font, .default, &glyph, nil, 1)
            var position = CGPoint(
                x: max(0, -bounds.minX),
                y: CTFontGetDescent(resolved.font)
            )
            CTFontDrawGlyphs(resolved.font, &glyph, &position, 1, context)
            return true
        }
        guard madeContext else {
            throw GlyphAtlasError.rasterizationFailed
        }
        return (bytes, width, height)
    }
}
