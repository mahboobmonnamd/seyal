import Foundation
import Metal

@MainActor
enum RendererValidation {
    static func deterministicSelfTest() -> Bool {
        guard let device = MTLCreateSystemDefaultDevice() else { return false }
        do {
            let renderer = try MetalTerminalRenderer(device: device)
            var cells = [
                preparedCell(background: terminalRGB(red: 255, green: 0, blue: 0)),
                preparedCell(background: terminalRGB(red: 0, green: 255, blue: 0)),
                preparedCell(background: terminalRGB(red: 0, green: 0, blue: 255)),
                preparedCell(background: terminalRGB(red: 255, green: 255, blue: 0)),
            ]
            var damage = DamageMask()
            damage.markAll(rows: 2)
            let orientationPassed = try cells.withUnsafeBufferPointer { buffer -> Bool in
                let frame = NativePreparedFrame(
                    cells: buffer,
                    generation: 1,
                    rows: 2,
                    columns: 2,
                    damage: damage
                )
                guard try renderer.update(
                    frame: frame,
                    backingScale: 1,
                    forceFullRebuild: true
                ) == .updated else {
                    return false
                }
                let cellSize = renderer.cellPixelSize(backingScale: 1)
                guard let texture = renderer.renderOffscreenAndWait(
                    width: cellSize.width * 2,
                    height: cellSize.height * 2
                ) else {
                    return false
                }
                return pixelMatches(
                    texture,
                    x: cellSize.width / 2,
                    y: cellSize.height / 2,
                    red: 255,
                    green: 0,
                    blue: 0
                ) && pixelMatches(
                    texture,
                    x: cellSize.width + cellSize.width / 2,
                    y: cellSize.height / 2,
                    red: 0,
                    green: 255,
                    blue: 0
                ) && pixelMatches(
                    texture,
                    x: cellSize.width / 2,
                    y: cellSize.height + cellSize.height / 2,
                    red: 0,
                    green: 0,
                    blue: 255
                ) && pixelMatches(
                    texture,
                    x: cellSize.width + cellSize.width / 2,
                    y: cellSize.height + cellSize.height / 2,
                    red: 255,
                    green: 255,
                    blue: 0
                )
            }
            guard orientationPassed else { return false }

            cells = [
                preparedCell(scalar: UInt32(ascii: "A"), foreground: terminalRGB(red: 240, green: 240, blue: 240)),
                preparedCell(scalar: UInt32(ascii: "A"), foreground: terminalRGB(red: 255, green: 0, blue: 0)),
            ]
            let beforeGlyphs = renderer.glyphStats
            var oneRow = DamageMask()
            oneRow.mark(row: 0)
            let glyphPassed = try cells.withUnsafeBufferPointer { buffer -> Bool in
                let frame = NativePreparedFrame(
                    cells: buffer,
                    generation: 2,
                    rows: 1,
                    columns: 2,
                    damage: oneRow
                )
                guard try renderer.update(
                    frame: frame,
                    backingScale: 1,
                    forceFullRebuild: true
                ) == .updated else {
                    return false
                }
                let after = renderer.glyphStats
                return after.uploads > beforeGlyphs.uploads && after.hits > beforeGlyphs.hits
            }
            guard glyphPassed, renderer.hasDedicatedSurfaceResources else { return false }

            let drawableMisses = renderer.stats.drawableMisses
            renderer.handleDrawableUnavailable()
            guard renderer.stats.drawableMisses == drawableMisses + 1 else { return false }

            renderer.setVisible(false)
            guard !renderer.hasDedicatedSurfaceResources else { return false }
            return GlyphAtlas.budgetBytes == 16 * 1024 * 1024
        } catch {
            return false
        }
    }

    static func liveSelfTest(expectAlternateScreen: Bool) -> Bool {
        guard let device = MTLCreateSystemDefaultDevice() else { return false }
        let connect = seyal_bridge_connect_first()
        guard connect == 0 else { return false }
        defer { seyal_bridge_disconnect() }

        do {
            let renderer = try MetalTerminalRenderer(device: device)
            let expected = expectAlternateScreen ? "ALT-LIVE" : "SEYAL-LIVE"
            let deadline = Date().addingTimeInterval(3)
            while Date() < deadline {
                let bridgeFrame = seyal_bridge_frame()
                if let frame = NativePreparedFrame(bridgeFrame: bridgeFrame),
                   (!expectAlternateScreen || frame.alternateScreen),
                   frameContains(frame, text: expected)
                {
                    guard try renderer.update(
                        frame: frame,
                        backingScale: 1,
                        forceFullRebuild: true
                    ) == .updated else {
                        return false
                    }
                    let cellSize = renderer.cellPixelSize(backingScale: 1)
                    guard let texture = renderer.renderOffscreenAndWait(
                        width: cellSize.width * frame.columns,
                        height: cellSize.height * frame.rows
                    ) else {
                        return false
                    }
                    return textureContainsBrightGlyphPixel(texture)
                }

                let poll = seyal_bridge_poll()
                if poll < 0 {
                    return false
                }
                Thread.sleep(forTimeInterval: 0.002)
            }
            return false
        } catch {
            return false
        }
    }

    static func runBenchmark() -> Bool {
        guard let device = MTLCreateSystemDefaultDevice() else { return false }
        do {
            let renderer = try MetalTerminalRenderer(device: device)
            let rows = 40
            let columns = 120
            let repetitions = 120
            var cells = [SeyalPreparedCell](
                repeating: preparedCell(scalar: UInt32(ascii: "a")),
                count: rows * columns
            )
            var full = DamageMask()
            full.markAll(rows: rows)
            try cells.withUnsafeBufferPointer { buffer in
                _ = try renderer.update(
                    frame: NativePreparedFrame(
                        cells: buffer,
                        generation: 1,
                        rows: rows,
                        columns: columns,
                        damage: full
                    ),
                    backingScale: 1,
                    forceFullRebuild: true
                )
            }

            let cellSize = renderer.cellPixelSize(backingScale: 1)
            var preparationSamples = [UInt64]()
            var gpuCompletionSamples = [UInt64]()
            preparationSamples.reserveCapacity(repetitions)
            gpuCompletionSamples.reserveCapacity(repetitions)

            for iteration in 0..<repetitions {
                let row = iteration % rows
                let index = row * columns
                cells[index].scalar = iteration.isMultiple(of: 2)
                    ? UInt32(ascii: "a")
                    : UInt32(ascii: "b")
                var damage = DamageMask()
                damage.mark(row: row)

                let prepareStarted = DispatchTime.now().uptimeNanoseconds
                try cells.withUnsafeBufferPointer { buffer in
                    _ = try renderer.update(
                        frame: NativePreparedFrame(
                            cells: buffer,
                            generation: UInt64(iteration + 2),
                            rows: rows,
                            columns: columns,
                            cursorRow: row,
                            cursorColumn: 0,
                            cursorVisible: true,
                            fullRebuild: false,
                            damage: damage
                        ),
                        backingScale: 1
                    )
                }
                preparationSamples.append(
                    DispatchTime.now().uptimeNanoseconds - prepareStarted
                )

                let gpuStarted = DispatchTime.now().uptimeNanoseconds
                guard renderer.renderOffscreenAndWait(
                    width: cellSize.width * columns,
                    height: cellSize.height * rows
                ) != nil else {
                    return false
                }
                gpuCompletionSamples.append(
                    DispatchTime.now().uptimeNanoseconds - gpuStarted
                )
            }

            let prep = percentileSummary(preparationSamples)
            let gpu = percentileSummary(gpuCompletionSamples)
            let glyph = renderer.glyphStats
            print("pass6_native_renderer performance_claim=false boundary=prepared_frame_to_metal_gpu_completion_proxy")
            print("device=\(device.name) registry_id=\(device.registryID) os=\(ProcessInfo.processInfo.operatingSystemVersionString) geometry=\(columns)x\(rows) repetitions=\(repetitions) backing_scale=1 percentile_method=nearest_rank")
            print("preparation p50_ns=\(prep.p50) p95_ns=\(prep.p95) p99_ns=\(prep.p99) max_ns=\(prep.max)")
            print("gpu_completion_proxy p50_ns=\(gpu.p50) p95_ns=\(gpu.p95) p99_ns=\(gpu.p99) max_ns=\(gpu.max) note=includes_offscreen_target_allocation")
            print("renderer submitted_frames=\(renderer.stats.submittedFrames) rebuilt_rows=\(renderer.stats.rebuiltRows) rebuilt_cells=\(renderer.stats.rebuiltCells) instance_bytes=\(renderer.stats.instanceBytes) glyph_hits=\(glyph.hits) glyph_misses=\(glyph.misses) glyph_uploads=\(glyph.uploads) glyph_uploaded_bytes=\(glyph.uploadedBytes) atlas_budget_bytes=\(GlyphAtlas.budgetBytes) dedicated_gpu_bytes=\(renderer.estimatedDedicatedGPUBytes)")
            return true
        } catch {
            return false
        }
    }

    private static func frameContains(_ frame: NativePreparedFrame, text: String) -> Bool {
        let scalars = text.unicodeScalars.map(\.value)
        guard !scalars.isEmpty, scalars.count <= frame.cells.count else { return false }
        var matched = 0
        for cell in frame.cells {
            if cell.scalar == scalars[matched] {
                matched += 1
                if matched == scalars.count {
                    return true
                }
            } else {
                matched = cell.scalar == scalars[0] ? 1 : 0
            }
        }
        return false
    }

    private static func textureContainsBrightGlyphPixel(_ texture: MTLTexture) -> Bool {
        let bytesPerRow = texture.width * 4
        var bytes = [UInt8](repeating: 0, count: bytesPerRow * texture.height)
        texture.getBytes(
            &bytes,
            bytesPerRow: bytesPerRow,
            from: MTLRegionMake2D(0, 0, texture.width, texture.height),
            mipmapLevel: 0
        )
        var offset = 0
        while offset + 3 < bytes.count {
            let blue = bytes[offset]
            let green = bytes[offset + 1]
            let red = bytes[offset + 2]
            if max(red, max(green, blue)) > 128 {
                return true
            }
            offset += 4
        }
        return false
    }

    private static func pixelMatches(
        _ texture: MTLTexture,
        x: Int,
        y: Int,
        red: UInt8,
        green: UInt8,
        blue: UInt8
    ) -> Bool {
        var pixel = [UInt8](repeating: 0, count: 4)
        texture.getBytes(
            &pixel,
            bytesPerRow: 4,
            from: MTLRegionMake2D(x, y, 1, 1),
            mipmapLevel: 0
        )
        return pixel[0] == blue
            && pixel[1] == green
            && pixel[2] == red
            && pixel[3] == 255
    }

    private static func preparedCell(
        scalar: UInt32 = 32,
        foreground: UInt32 = 0,
        background: UInt32 = 0,
        flags: UInt16 = 0
    ) -> SeyalPreparedCell {
        var cell = SeyalPreparedCell()
        cell.scalar = scalar
        cell.foreground = foreground
        cell.background = background
        cell.flags = flags
        cell.reserved = 0
        return cell
    }

    private static func terminalRGB(red: UInt8, green: UInt8, blue: UInt8) -> UInt32 {
        0x0200_0000
            | (UInt32(red) << 16)
            | (UInt32(green) << 8)
            | UInt32(blue)
    }

    private static func percentileSummary(_ input: [UInt64]) -> (p50: UInt64, p95: UInt64, p99: UInt64, max: UInt64) {
        let samples = input.sorted()
        return (
            percentile(samples, 50),
            percentile(samples, 95),
            percentile(samples, 99),
            samples.last ?? 0
        )
    }

    private static func percentile(_ samples: [UInt64], _ value: Int) -> UInt64 {
        guard !samples.isEmpty else { return 0 }
        let rank = max(1, (samples.count * value + 99) / 100)
        return samples[min(rank - 1, samples.count - 1)]
    }
}

private extension UInt32 {
    init(ascii character: Character) {
        self = character.unicodeScalars.first?.value ?? 0x20
    }
}
