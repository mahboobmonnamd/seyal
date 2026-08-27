import Foundation
import AppKit
import Metal
@preconcurrency import QuartzCore

@MainActor
private final class DisplayLinkBenchmarkDriver: NSObject, @preconcurrency CAMetalDisplayLinkDelegate {
    private let renderer: MetalTerminalRenderer
    private let link: CAMetalDisplayLink
    private var startedAt: UInt64?
    private(set) var samples = [UInt64]()

    init(renderer: MetalTerminalRenderer, layer: CAMetalLayer) {
        self.renderer = renderer
        link = CAMetalDisplayLink(metalLayer: layer)
        super.init()
        link.delegate = self
        link.isPaused = true
        link.add(to: .main, forMode: .common)
    }

    func submitOne() -> Bool {
        guard startedAt == nil else { return false }
        renderer.requestPresent()
        startedAt = DispatchTime.now().uptimeNanoseconds
        link.isPaused = false
        let sampleCount = samples.count
        let deadline = Date().addingTimeInterval(2)
        while samples.count == sampleCount, Date() < deadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.001))
        }
        let receivedSample = samples.count == sampleCount + 1
        if !receivedSample {
            // A headless macOS runner may have no WindowServer/display session,
            // so no CAMetalDisplayLink opportunity can arrive. Do not leave a
            // stale in-flight request blocking later benchmark iterations.
            startedAt = nil
        }
        return receivedSample
    }

    func metalDisplayLink(
        _ link: CAMetalDisplayLink,
        needsUpdate update: CAMetalDisplayLink.Update
    ) {
        link.isPaused = true
        guard let startedAt,
              renderer.present(drawable: update.drawable)
        else {
            self.startedAt = nil
            return
        }
        samples.append(DispatchTime.now().uptimeNanoseconds - startedAt)
        self.startedAt = nil
    }

    deinit {
        link.delegate = nil
        link.invalidate()
    }
}

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
                preparedCell(
                    scalar: UInt32(ascii: "A"),
                    foreground: terminalRGB(red: 240, green: 240, blue: 240)
                ),
                preparedCell(
                    scalar: UInt32(ascii: "A"),
                    foreground: terminalRGB(red: 255, green: 0, blue: 0)
                ),
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

            // Bold is a renderer/cache identity seam in M001. It must be able to
            // resolve different raster pixels without making terminal color part
            // of glyph identity.
            let beforeBold = renderer.glyphStats
            cells = [preparedCell(scalar: UInt32(ascii: "A"), flags: 1)]
            guard try cells.withUnsafeBufferPointer({ buffer in
                try renderer.update(
                    frame: NativePreparedFrame(
                        cells: buffer,
                        generation: 3,
                        rows: 1,
                        columns: 1,
                        damage: oneRow
                    ),
                    backingScale: 1,
                    forceFullRebuild: true
                ) == .updated
            }) else {
                return false
            }
            guard renderer.glyphStats.uploads > beforeBold.uploads else { return false }

            // Blank backgrounds, underline geometry and cursor inversion all use
            // the same cell rectangle and must render without relying on glyphs.
            cells = [
                preparedCell(
                    foreground: terminalRGB(red: 255, green: 0, blue: 0),
                    background: terminalRGB(red: 0, green: 0, blue: 255),
                    flags: 2
                ),
                preparedCell(
                    foreground: terminalRGB(red: 0, green: 255, blue: 0),
                    background: terminalRGB(red: 255, green: 0, blue: 0)
                ),
            ]
            let stylePassed = try cells.withUnsafeBufferPointer { buffer -> Bool in
                let frame = NativePreparedFrame(
                    cells: buffer,
                    generation: 4,
                    rows: 1,
                    columns: 2,
                    cursorRow: 0,
                    cursorColumn: 1,
                    cursorVisible: true,
                    damage: oneRow
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
                    height: cellSize.height
                ) else {
                    return false
                }
                let underlineY = max(0, cellSize.height - 1)
                return pixelMatches(
                    texture,
                    x: cellSize.width / 2,
                    y: cellSize.height / 2,
                    red: 0,
                    green: 0,
                    blue: 255
                ) && pixelMatches(
                    texture,
                    x: cellSize.width / 2,
                    y: underlineY,
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
                )
            }
            guard stylePassed else { return false }

            let fullRebuildsBeforeScale = renderer.stats.fullRebuilds
            let atlasResetsBeforeScale = renderer.glyphStats.resets
            guard try cells.withUnsafeBufferPointer({ buffer in
                try renderer.update(
                    frame: NativePreparedFrame(
                        cells: buffer,
                        generation: 5,
                        rows: 1,
                        columns: 2,
                        cursorRow: 0,
                        cursorColumn: 1,
                        cursorVisible: true,
                        fullRebuild: false,
                        damage: DamageMask()
                    ),
                    backingScale: 2
                ) == .updated
            }) else {
                return false
            }
            guard renderer.stats.fullRebuilds > fullRebuildsBeforeScale,
                  renderer.glyphStats.resets > atlasResetsBeforeScale
            else {
                return false
            }

            // Hide must release dedicated resources. Showing again requests the
            // latest committed frame rather than replaying PTY bytes and forces a
            // reconstructable full redraw.
            renderer.setVisible(false)
            guard !renderer.hasDedicatedSurfaceResources else { return false }
            var currentFrameRequests = 0
            renderer.onNeedsCurrentFrame = { currentFrameRequests += 1 }
            renderer.setVisible(true)
            guard currentFrameRequests == 1 else { return false }
            let fullRebuildsBeforeShow = renderer.stats.fullRebuilds
            guard try cells.withUnsafeBufferPointer({ buffer in
                try renderer.update(
                    frame: NativePreparedFrame(
                        cells: buffer,
                        generation: 6,
                        rows: 1,
                        columns: 2,
                        cursorRow: 0,
                        cursorColumn: 1,
                        cursorVisible: true,
                        fullRebuild: false,
                        damage: DamageMask()
                    ),
                    backingScale: 1
                ) == .updated
            }) else {
                return false
            }
            guard renderer.stats.fullRebuilds > fullRebuildsBeforeShow else { return false }
            renderer.setVisible(false)
            guard !renderer.hasDedicatedSurfaceResources else { return false }

            // Offscreen tests above prove deterministic pixels. These separate
            // proofs cover finite atlas pressure/reclamation, repeated surface
            // lifecycle cleanup, and the production CAMetalLayer path itself.
            guard atlasPressureSelfTest(device: device),
                  try repeatedLifecycleSelfTest(device: device),
                  try productionLayerPresentSelfTest(device: device)
            else {
                return false
            }
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
                    ), textureContainsBrightGlyphPixel(texture) else {
                        return false
                    }

                    // The same real Candidate-D-prepared state must also be
                    // accepted by the production CAMetalLayer presentation path.
                    let layer = makePresentationLayer(
                        device: device,
                        width: cellSize.width * frame.columns,
                        height: cellSize.height * frame.rows
                    )
                    let completedBefore = renderer.stats.completedFrames
                    let submittedBefore = renderer.stats.submittedFrames
                    guard presentOnLayerForValidation(renderer: renderer, layer: layer),
                          renderer.stats.submittedFrames == submittedBefore + 1
                    else {
                        return false
                    }
                    return waitForGPUCompletion(renderer, after: completedBefore)
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
            let presentationLayer = makePresentationLayer(
                device: device,
                width: cellSize.width * columns,
                height: cellSize.height * rows
            )
            // CAMetalDisplayLink only produces frame opportunities for a layer
            // participating in an AppKit window hierarchy. Keep this small
            // benchmark window visible so the measurement exercises the same
            // scheduler/layer contract as production.
            let application = NSApplication.shared
            application.setActivationPolicy(.accessory)
            application.finishLaunching()
            application.activate(ignoringOtherApps: true)
            let benchmarkWindow = NSWindow(
                contentRect: NSRect(
                    x: 0,
                    y: 0,
                    width: cellSize.width * columns,
                    height: cellSize.height * rows
                ),
                styleMask: [.borderless],
                backing: .buffered,
                defer: false
            )
            let benchmarkHost = NSView(frame: benchmarkWindow.contentRect(forFrameRect: benchmarkWindow.frame))
            benchmarkHost.wantsLayer = true
            benchmarkHost.layer?.addSublayer(presentationLayer)
            benchmarkWindow.contentView = benchmarkHost
            benchmarkWindow.makeKeyAndOrderFront(nil)
            let displayLinkDriver = DisplayLinkBenchmarkDriver(
                renderer: renderer,
                layer: presentationLayer
            )
            var preparationSamples = [UInt64]()
            var preparedToCommitSamples = [UInt64]()
            var commitToCompletionSamples = [UInt64]()
            preparationSamples.reserveCapacity(repetitions)
            preparedToCommitSamples.reserveCapacity(repetitions)
            commitToCompletionSamples.reserveCapacity(repetitions)
            var presentationProxyAvailable = true
            let presentationProxyRequired = ProcessInfo.processInfo.environment[
                "SEYAL_REQUIRE_DISPLAY_LINK_BENCHMARK"
            ] == "1"

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

                guard let submission = renderer.renderOffscreenAndMeasureSubmission(
                    width: cellSize.width * columns,
                    height: cellSize.height * rows
                ) else {
                    return false
                }
                preparedToCommitSamples.append(submission.preparedToCommitNanoseconds)
                commitToCompletionSamples.append(submission.commitToCompletionNanoseconds)
                if presentationProxyAvailable, !displayLinkDriver.submitOne() {
                    guard presentationProxyRequired else {
                        presentationProxyAvailable = false
                        continue
                    }
                    return false
                }
            }

            let prep = percentileSummary(preparationSamples)
            let preparedToCommit = percentileSummary(preparedToCommitSamples)
            let commitToCompletion = percentileSummary(commitToCompletionSamples)
            let glyph = renderer.glyphStats
            print("pass6_native_renderer performance_claim=false boundaries=committed_generation_to_prepared_rows,prepared_batch_to_command_commit,command_commit_to_gpu_completion,committed_generation_to_presented_frame_proxy")
            print("device=\(device.name) registry_id=\(device.registryID) os=\(ProcessInfo.processInfo.operatingSystemVersionString) geometry=\(columns)x\(rows) repetitions=\(repetitions) backing_scale=1 percentile_method=nearest_rank")
            print("preparation p50_ns=\(prep.p50) p95_ns=\(prep.p95) p99_ns=\(prep.p99) max_ns=\(prep.max)")
            print("prepared_to_command_commit p50_ns=\(preparedToCommit.p50) p95_ns=\(preparedToCommit.p95) p99_ns=\(preparedToCommit.p99) max_ns=\(preparedToCommit.max) note=offscreen_target_allocation_excluded")
            print("command_commit_to_gpu_completion_proxy p50_ns=\(commitToCompletion.p50) p95_ns=\(commitToCompletion.p95) p99_ns=\(commitToCompletion.p99) max_ns=\(commitToCompletion.max)")
            if presentationProxyAvailable {
                let presented = percentileSummary(displayLinkDriver.samples)
                print("committed_generation_to_presented_frame_proxy p50_ns=\(presented.p50) p95_ns=\(presented.p95) p99_ns=\(presented.p99) max_ns=\(presented.max) note=one_shot_CAMetalDisplayLink_to_command_commit")
            } else {
                print("committed_generation_to_presented_frame_proxy status=PLATFORM_LIMITED samples=0 reason=no_WindowServer_display_session")
            }
            print("renderer submitted_frames=\(renderer.stats.submittedFrames) display_link_samples=\(displayLinkDriver.samples.count) coalesced_frames=\(renderer.stats.coalescedFrames) rebuilt_rows=\(renderer.stats.rebuiltRows) rebuilt_cells=\(renderer.stats.rebuiltCells) instance_bytes=\(renderer.stats.instanceBytes) glyph_hits=\(glyph.hits) glyph_misses=\(glyph.misses) glyph_uploads=\(glyph.uploads) glyph_uploaded_bytes=\(glyph.uploadedBytes) atlas_budget_bytes=\(GlyphAtlas.budgetBytes) dedicated_gpu_bytes=\(renderer.estimatedDedicatedGPUBytes)")
            benchmarkWindow.orderOut(nil)
            return true
        } catch {
            return false
        }
    }

    private static func atlasPressureSelfTest(device: MTLDevice) -> Bool {
        let atlas = GlyphAtlas(device: device)
        let pressureScale: CGFloat = 48
        let pressureMetrics = atlas.metrics(backingScale: pressureScale)
        var reachedCapacity = false

        // At this controlled scale the finite 2048x2048x4 atlas can hold fewer
        // than the printable ASCII glyph set, so pressure is deterministic and
        // does not require thousands of platform-font rasterizations.
        for scalar in UInt32(0x21)...UInt32(0x7e) {
            do {
                _ = try atlas.lookup(
                    scalar: scalar,
                    bold: false,
                    backingScale: pressureScale,
                    cellMetrics: pressureMetrics
                )
            } catch GlyphAtlasError.capacityExceeded {
                reachedCapacity = true
                break
            } catch {
                return false
            }
        }
        guard reachedCapacity,
              atlas.estimatedResidentBytes == GlyphAtlas.budgetBytes,
              atlas.entryCount > 0
        else {
            return false
        }

        let resetsBefore = atlas.stats.resets
        atlas.resetWhenGPUIdle()
        guard atlas.stats.resets == resetsBefore + 1,
              atlas.entryCount == 0,
              atlas.estimatedResidentBytes == 0
        else {
            return false
        }

        do {
            let normalMetrics = atlas.metrics(backingScale: 1)
            _ = try atlas.lookup(
                scalar: UInt32(ascii: "A"),
                bold: false,
                backingScale: 1,
                cellMetrics: normalMetrics
            )
            return atlas.entryCount == 1
                && atlas.estimatedResidentBytes == GlyphAtlas.budgetBytes
        } catch {
            return false
        }
    }

    private static func repeatedLifecycleSelfTest(device: MTLDevice) throws -> Bool {
        var damage = DamageMask()
        damage.mark(row: 0)
        let cells = [preparedCell(scalar: UInt32(ascii: "A"))]

        for generation in 1...8 {
            weak var releasedRenderer: MetalTerminalRenderer?
            do {
                let renderer = try MetalTerminalRenderer(device: device)
                releasedRenderer = renderer
                renderer.setVisible(false)
                guard !renderer.hasDedicatedSurfaceResources,
                      renderer.estimatedDedicatedGPUBytes == 0
                else {
                    return false
                }

                renderer.setVisible(true)
                guard try cells.withUnsafeBufferPointer({ buffer in
                    try renderer.update(
                        frame: NativePreparedFrame(
                            cells: buffer,
                            generation: UInt64(generation),
                            rows: 1,
                            columns: 1,
                            damage: damage
                        ),
                        backingScale: 1,
                        forceFullRebuild: true
                    ) == .updated
                }), renderer.hasDedicatedSurfaceResources else {
                    return false
                }

                renderer.setVisible(false)
                guard !renderer.hasDedicatedSurfaceResources,
                      renderer.estimatedDedicatedGPUBytes == 0
                else {
                    return false
                }
            }
            guard releasedRenderer == nil else { return false }
        }
        return true
    }

    private static func productionLayerPresentSelfTest(device: MTLDevice) throws -> Bool {
        let renderer = try MetalTerminalRenderer(device: device)
        var damage = DamageMask()
        damage.mark(row: 0)
        let cells = [preparedCell(scalar: UInt32(ascii: "A"))]
        guard try cells.withUnsafeBufferPointer({ buffer in
            try renderer.update(
                frame: NativePreparedFrame(
                    cells: buffer,
                    generation: 1,
                    rows: 1,
                    columns: 1,
                    damage: damage
                ),
                backingScale: 1,
                forceFullRebuild: true
            ) == .updated
        }) else {
            return false
        }
        let cellSize = renderer.cellPixelSize(backingScale: 1)
        let layer = makePresentationLayer(
            device: device,
            width: cellSize.width,
            height: cellSize.height
        )
        let completedBefore = renderer.stats.completedFrames
        guard presentOnLayerForValidation(renderer: renderer, layer: layer), renderer.stats.submittedFrames == 1 else {
            return false
        }

        // While the submitted command buffer can still reference the existing
        // atlas/instance resources, a scale-invalidating update must coalesce
        // rather than reset/reclaim them. MainActor serialization makes this
        // check deterministic: the completion task cannot run until we pump the
        // run loop below.
        let resetsWhileInFlight = renderer.glyphStats.resets
        let deferred = try cells.withUnsafeBufferPointer { buffer in
            try renderer.update(
                frame: NativePreparedFrame(
                    cells: buffer,
                    generation: 2,
                    rows: 1,
                    columns: 1,
                    fullRebuild: true,
                    damage: damage
                ),
                backingScale: 2
            )
        }
        guard deferred == .deferred,
              renderer.glyphStats.resets == resetsWhileInFlight,
              waitForGPUCompletion(renderer, after: completedBefore)
        else {
            return false
        }

        let resetsAfterCompletion = renderer.glyphStats.resets
        let updated = try cells.withUnsafeBufferPointer { buffer in
            try renderer.update(
                frame: NativePreparedFrame(
                    cells: buffer,
                    generation: 3,
                    rows: 1,
                    columns: 1,
                    fullRebuild: true,
                    damage: damage
                ),
                backingScale: 2
            )
        }
        return updated == .updated && renderer.glyphStats.resets > resetsAfterCompletion
    }

    static func inFlightVisibilityRecoverySelfTest() -> Bool {
        guard let device = MTLCreateSystemDefaultDevice() else { return false }
        do {
            let renderer = try MetalTerminalRenderer(device: device)
            let cells = [preparedCell(scalar: UInt32(ascii: "A"))]
            var damage = DamageMask()
            damage.mark(row: 0)
            guard try cells.withUnsafeBufferPointer({ buffer in
                try renderer.update(
                    frame: NativePreparedFrame(
                        cells: buffer,
                        generation: 1,
                        rows: 1,
                        columns: 1,
                        damage: damage
                    ),
                    backingScale: 1,
                    forceFullRebuild: true
                ) == .updated
            }) else { return false }

            let cellSize = renderer.cellPixelSize(backingScale: 1)
            let layer = makePresentationLayer(
                device: device,
                width: cellSize.width,
                height: cellSize.height
            )
            var currentFrameRequests = 0
            renderer.onNeedsCurrentFrame = { currentFrameRequests += 1 }
            let completedBefore = renderer.stats.completedFrames
            guard presentOnLayerForValidation(renderer: renderer, layer: layer), renderer.hasFrameInFlight else {
                return false
            }
            renderer.setVisible(false)
            renderer.setVisible(true)
            return waitForGPUCompletion(renderer, after: completedBefore)
                && currentFrameRequests == 1
                && renderer.hasDedicatedSurfaceResources
        } catch {
            return false
        }
    }

    static func failedReplacementInvalidationSelfTest() -> Bool {
        guard let device = MTLCreateSystemDefaultDevice() else { return false }
        do {
            let renderer = try MetalTerminalRenderer(device: device)
            var cells = [
                preparedCell(scalar: UInt32(ascii: "A")),
                preparedCell(scalar: UInt32(ascii: "B"))
            ]
            var damage = DamageMask()
            damage.mark(row: 0)
            guard try cells.withUnsafeBufferPointer({ buffer in
                try renderer.update(
                    frame: NativePreparedFrame(
                        cells: buffer,
                        generation: 1,
                        rows: 1,
                        columns: 2,
                        damage: damage
                    ),
                    backingScale: 1,
                    forceFullRebuild: true
                ) == .updated
            }), renderer.hasPresentablePreparedState else { return false }

            cells[1].reserved = 1
            do {
                _ = try cells.withUnsafeBufferPointer { buffer in
                    try renderer.update(
                        frame: NativePreparedFrame(
                            cells: buffer,
                            generation: 2,
                            rows: 1,
                            columns: 2,
                            damage: damage
                        ),
                        backingScale: 1,
                        forceFullRebuild: true
                    )
                }
                return false
            } catch {
                guard !renderer.hasPresentablePreparedState else { return false }
            }

            cells[1].reserved = 0
            return try cells.withUnsafeBufferPointer { buffer in
                try renderer.update(
                    frame: NativePreparedFrame(
                        cells: buffer,
                        generation: 3,
                        rows: 1,
                        columns: 2,
                        damage: damage
                    ),
                    backingScale: 1,
                    forceFullRebuild: true
                ) == .updated && renderer.hasPresentablePreparedState
            }
        } catch {
            return false
        }
    }

    private static func makePresentationLayer(
        device: MTLDevice,
        width: Int,
        height: Int
    ) -> CAMetalLayer {
        let layer = CAMetalLayer()
        layer.device = device
        layer.pixelFormat = .bgra8Unorm
        layer.framebufferOnly = true
        layer.maximumDrawableCount = 2
        layer.presentsWithTransaction = false
        layer.contentsScale = 1
        layer.bounds = CGRect(x: 0, y: 0, width: width, height: height)
        layer.drawableSize = CGSize(width: width, height: height)
        return layer
    }

    private static func waitForGPUCompletion(
        _ renderer: MetalTerminalRenderer,
        after completedBefore: UInt64
    ) -> Bool {
        let deadline = Date().addingTimeInterval(2)
        while renderer.stats.completedFrames == completedBefore && Date() < deadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.01))
        }
        return renderer.stats.completedFrames > completedBefore
    }

    static func presentOnLayerForValidation(
        renderer: MetalTerminalRenderer,
        layer: CAMetalLayer
    ) -> Bool {
        guard let drawable = layer.nextDrawable() else { return false }
        return renderer.present(drawable: drawable)
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
