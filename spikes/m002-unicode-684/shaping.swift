import CoreText
import Foundation

struct Sample {
    let label: String
    let text: String
    let terminalWidth: Int
}

struct ShapeSummary {
    let runCount: Int
    let glyphCount: Int
    let typographicWidth: Double
    let fonts: [String]
}

let samples: [Sample] = [
    Sample(label: "ascii", text: "A", terminalWidth: 1),
    Sample(label: "combining", text: "e\u{301}", terminalWidth: 1),
    Sample(label: "cjk-wide", text: "界", terminalWidth: 2),
    Sample(label: "emoji-vs16", text: "❤\u{fe0f}", terminalWidth: 2),
    Sample(label: "emoji-zwj", text: "👩‍💻", terminalWidth: 2),
    Sample(label: "emoji-family", text: "👨‍👩‍👧‍👦", terminalWidth: 2),
    Sample(label: "regional-flag", text: "🇮🇳", terminalWidth: 2),
    Sample(label: "tamil-combining", text: "நி", terminalWidth: 2),
    Sample(label: "arabic-combining", text: "نِ", terminalWidth: 1),
    Sample(label: "supplementary-plane", text: "𐐷", terminalWidth: 1),
]

let pointSize: CGFloat = 16
let baseFont = CTFontCreateWithName("Menlo" as CFString, pointSize, nil)
let fontKey = NSAttributedString.Key(kCTFontAttributeName as String)

func shape(_ text: String) -> ShapeSummary {
    let attributed = NSAttributedString(
        string: text,
        attributes: [fontKey: baseFont]
    )
    let line = CTLineCreateWithAttributedString(attributed as CFAttributedString)
    let runs = CTLineGetGlyphRuns(line) as! [CTRun]
    var glyphCount = 0
    var fontNames = Set<String>()

    for run in runs {
        glyphCount += CTRunGetGlyphCount(run)
        let attributes = CTRunGetAttributes(run) as NSDictionary
        if let runFont = attributes.object(forKey: kCTFontAttributeName) as? CTFont {
            fontNames.insert(CTFontCopyPostScriptName(runFont) as String)
        }
    }

    return ShapeSummary(
        runCount: runs.count,
        glyphCount: glyphCount,
        typographicWidth: CTLineGetTypographicBounds(line, nil, nil, nil),
        fonts: fontNames.sorted()
    )
}

func elapsedNanoseconds(_ body: () -> Int) -> (UInt64, Int) {
    let start = DispatchTime.now().uptimeNanoseconds
    let checksum = body()
    return (DispatchTime.now().uptimeNanoseconds - start, checksum)
}

let arch = ProcessInfo.processInfo.environment["RUNNER_ARCH"] ?? "unknown"
let os = ProcessInfo.processInfo.operatingSystemVersionString
var mGlyph: CGGlyph = 0
var mCharacter = UniChar(77)
_ = CTFontGetGlyphsForCharacters(baseFont, &mCharacter, &mGlyph, 1)
var mAdvance = CGSize.zero
_ = CTFontGetAdvancesForGlyphs(baseFont, .horizontal, &mGlyph, &mAdvance, 1)

print("SHAPING\tnote\tderived-renderer evidence only; typographic width is not terminal width authority")
print("SHAPING\trunner_arch\t\(arch)\tos\t\(os)")
print("SHAPING\tbase_font\t\(CTFontCopyPostScriptName(baseFont) as String)\tpoint_size\t\(pointSize)\tm_advance\t\(mAdvance.width)")
print("SHAPE\tlabel\tutf8_bytes\tterminal_cells\truns\tglyphs\ttypographic_width\tfonts")

for sample in samples {
    let summary = shape(sample.text)
    print(
        "SHAPE\t\(sample.label)\t\(sample.text.utf8.count)\t\(sample.terminalWidth)\t\(summary.runCount)\t\(summary.glyphCount)\t\(String(format: \"%.3f\", summary.typographicWidth))\t\(summary.fonts.joined(separator: \",\"))"
    )
}

let rounds = 2_000
for sample in samples {
    for _ in 0..<64 {
        _ = shape(sample.text)
    }

    let (coldishNs, shapeChecksum) = elapsedNanoseconds {
        var checksum = 0
        for _ in 0..<rounds {
            let result = shape(sample.text)
            checksum &+= result.glyphCount
            checksum &+= result.runCount
        }
        return checksum
    }

    let cached = shape(sample.text)
    var cache = [sample.text: cached]
    let (cacheNs, cacheChecksum) = elapsedNanoseconds {
        var checksum = 0
        for _ in 0..<rounds {
            if let result = cache[sample.text] {
                checksum &+= result.glyphCount
                checksum &+= result.runCount
            }
        }
        return checksum
    }
    cache.removeAll(keepingCapacity: false)

    print(
        "SHAPE_BENCH\t\(sample.label)\trounds=\(rounds)\tshape_total_ns=\(coldishNs)\tshape_ns_per_op=\(Double(coldishNs) / Double(rounds))\tcache_total_ns=\(cacheNs)\tcache_ns_per_op=\(Double(cacheNs) / Double(rounds))\tchecksums=\(shapeChecksum),\(cacheChecksum)"
    )
}
