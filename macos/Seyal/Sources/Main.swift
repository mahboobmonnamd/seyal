import AppKit
import Darwin
import Metal

@main
enum SeyalMain {
    @MainActor
    static func main() {
        if CommandLine.arguments.contains("--smoke-test") {
            #if DEBUG
            guard MetalSurfaceView.smokeTest(), SeyalShellView.smokeTest() else {
                print("Seyal native smoke test failed.")
                exit(1)
            }
            print("Seyal native Swift/AppKit/Metal + UI shell smoke test passed.")
            #else
            guard MetalSurfaceView.smokeTest() else {
                print("Seyal native smoke test failed.")
                exit(1)
            }
            print("Seyal native Swift/AppKit/Metal smoke test passed.")
            #endif
            return
        }

        if CommandLine.arguments.contains("--renderer-self-test") {
            guard RendererValidation.deterministicSelfTest(),
                  Pass6RegressionValidation.selfTest(),
                  MetalTerminalRenderer.gpuCompletionFailureRecoverySelfTest(),
                  InteractiveMetalSurfaceView.pass7InputSelfTest(),
                  RuntimeLifecycleRecoveryCoordinator.ownershipSelfTest()
            else {
                print("Seyal deterministic Metal renderer/input/recovery self-test failed.")
                exit(1)
            }
            print("Seyal deterministic Metal renderer/input/recovery self-test passed.")
            return
        }

        if CommandLine.arguments.contains("--renderer-live-self-test") {
            let expectAlternate = CommandLine.arguments.contains("--expect-alternate")
            guard RendererValidation.liveSelfTest(expectAlternateScreen: expectAlternate) else {
                print("Seyal live Candidate-D to Metal renderer self-test failed.")
                exit(1)
            }
            print("Seyal live Candidate-D to Metal renderer self-test passed.")
            return
        }

        if CommandLine.arguments.contains("--pass8-native-metadata-self-test") {
            guard pass8NativeMetadataSelfTest() else {
                print("Seyal Pass 8 Runtime-to-Swift metadata self-test failed.")
                exit(1)
            }
            print("Seyal Pass 8 Runtime-to-Swift metadata self-test passed.")
            return
        }

        if CommandLine.arguments.contains("--pass7-native-input-benchmark") {
            guard runPass7NativeInputBenchmark() else {
                print("Seyal Pass 7 native input benchmark failed.")
                exit(1)
            }
            return
        }

        if CommandLine.arguments.contains("--pass9-renderer-calibration") {
            guard runPass9RendererCalibration() else {
                print("Seyal Pass 9 renderer lifecycle calibration failed.")
                exit(1)
            }
            return
        }

        if CommandLine.arguments.contains("--pass9-merge-acceptance") {
            guard Pass9MergeAcceptance.run() else {
                print("Seyal Pass 9 merge-acceptance soak failed.")
                exit(1)
            }
            return
        }

        if CommandLine.arguments.contains("--pass9-release-qualification") {
            guard Pass9ReleaseQualification.run() else {
                print("Seyal Pass 9 release-qualification soak failed.")
                exit(1)
            }
            return
        }

        if CommandLine.arguments.contains("--pass9-input-accessibility-qualification") {
            guard Pass9InputAccessibilityQualification.run() else {
                print("Seyal Pass 9 input/accessibility qualification failed.")
                exit(1)
            }
            return
        }

        if CommandLine.arguments.contains("--renderer-benchmark") {
            guard RendererValidation.runBenchmark() else {
                print("Seyal native renderer benchmark failed.")
                exit(1)
            }
            return
        }

        let application = NSApplication.shared
        let delegate = AppDelegate()
        application.delegate = delegate
        application.setActivationPolicy(.regular)
        application.run()
        withExtendedLifetime(delegate) {}
    }

    @MainActor
    private static func pass8NativeMetadataSelfTest() -> Bool {
        let application = NSApplication.shared
        application.setActivationPolicy(.prohibited)

        let bridge = RustDisplayBridge(
            onFrame: { _ in },
            onError: { _ in },
            paneID: "pass8-native-self-test"
        )
        let deadline = Date().addingTimeInterval(2)
        while !bridge.start() && Date() < deadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.01))
        }
        guard bridge.isConnected else { return false }
        defer { bridge.stop() }

        repeat {
            guard bridge.clientHandle != 0,
                  seyal_bridge_select(bridge.clientHandle) == 0
            else { return false }

            let pollResult = seyal_bridge_poll()
            guard pollResult >= 0 else { return false }

            if let metadata = bridge.currentBlockMetadata() {
                return (metadata.blockIDLow != 0 || metadata.blockIDHigh != 0)
                    && metadata.revision == 1
                    && metadata.startLineID > 0
                    && metadata.state == .current
            }
            RunLoop.current.run(until: Date().addingTimeInterval(0.01))
        } while Date() < deadline

        return false
    }

    @MainActor
    private static func runPass7NativeInputBenchmark() -> Bool {
        let repetitions = 120
        let warmups = 20
        let application = NSApplication.shared
        application.setActivationPolicy(.prohibited)

        let contentRect = NSRect(x: 0, y: 0, width: 960, height: 600)
        let surface = InteractiveMetalSurfaceView(frame: contentRect)
        let window = NSWindow(
            contentRect: contentRect,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        window.contentView = surface
        window.makeKeyAndOrderFront(nil)
        defer { window.orderOut(nil) }
        guard window.makeFirstResponder(surface) else { return false }

        // Pass 9 production startup is visibility-driven and coordinator-owned;
        // benchmark setup must exercise that same path instead of relying on a
        // synchronous bridge.start() side effect from surface construction.
        let recoveryDeadline = Date().addingTimeInterval(2)
        while !surface.terminalBridgeIsConnected && Date() < recoveryDeadline {
            RunLoop.current.run(until: Date().addingTimeInterval(0.01))
        }
        guard surface.terminalBridgeIsConnected else { return false }

        func makeReturnEvent() -> NSEvent? {
            NSEvent.keyEvent(
                with: .keyDown,
                location: .zero,
                modifierFlags: [],
                timestamp: ProcessInfo.processInfo.systemUptime,
                windowNumber: window.windowNumber,
                context: nil,
                characters: "\r",
                charactersIgnoringModifiers: "\r",
                isARepeat: false,
                keyCode: 36
            )
        }

        for _ in 0..<warmups {
            guard let event = makeReturnEvent() else { return false }
            surface.keyDown(with: event)
            guard surface.terminalInputFailureCode() == 0 else { return false }
        }

        var samples = [UInt64]()
        samples.reserveCapacity(repetitions)
        for _ in 0..<repetitions {
            guard let event = makeReturnEvent() else { return false }
            let started = DispatchTime.now().uptimeNanoseconds
            surface.keyDown(with: event)
            let finished = DispatchTime.now().uptimeNanoseconds
            guard surface.terminalInputFailureCode() == 0, finished >= started else {
                return false
            }
            samples.append(finished - started)
        }
        samples.sort()

        func percentile(_ value: Int) -> UInt64 {
            guard !samples.isEmpty else { return 0 }
            let rank = max(1, (value * samples.count + 99) / 100)
            return samples[min(rank - 1, samples.count - 1)]
        }

        let p50 = Double(percentile(50)) / 1_000.0
        let p95 = Double(percentile(95)) / 1_000.0
        let p99 = Double(percentile(99)) / 1_000.0
        let maximum = Double(samples.last ?? 0) / 1_000.0
        print(
            "pass7_native_input boundary=synthetic_NSEvent_to_production_keyDown_return "
                + "classification=MEASURED sample_count=\(samples.count) "
                + "p50_us=\(String(format: "%.3f", p50)) "
                + "p95_us=\(String(format: "%.3f", p95)) "
                + "p99_us=\(String(format: "%.3f", p99)) "
                + "max_us=\(String(format: "%.3f", maximum)) "
                + "appkit_event_boundary=true production_keyDown_route=true "
                + "synthetic_event=true physical_keyboard=false performance_claim=false"
        )
        return true
    }

    @MainActor
    private static func runPass9RendererCalibration() -> Bool {
        guard let device = MTLCreateSystemDefaultDevice() else { return false }
        let warmups = 20
        let measuredCycles = 100
        let cohorts = 5
        let geometries = [(columns: 120, rows: 40), (columns: 80, rows: 24)]

        func makeCell() -> SeyalPreparedCell {
            var cell = SeyalPreparedCell()
            cell.scalar = 0x61
            cell.foreground = 0
            cell.background = 0
            cell.flags = 0
            cell.reserved = 0
            return cell
        }

        func percentile(_ sorted: [UInt64], _ value: Int) -> UInt64 {
            guard !sorted.isEmpty else { return 0 }
            let rank = max(1, (sorted.count * value + 99) / 100)
            return sorted[min(rank - 1, sorted.count - 1)]
        }

        print(
            "pass9_renderer_calibration architecture=production_MetalTerminalRenderer "
                + "warmup_cycles=\(warmups) measured_cycles=\(measuredCycles) cohorts=\(cohorts) "
                + "geometries=120x40,80x24 resource_boundary=setVisible_false_completed "
                + "resource_gate=dedicated_GPU_bytes_and_surface_resources_zero_every_cycle "
                + "presentation_completion=NOT_CLAIMED performance_claim=false"
        )

        do {
            for geometry in geometries {
                for cohort in 1...cohorts {
                    let renderer = try MetalTerminalRenderer(device: device)
                    renderer.setVisible(false)
                    guard !renderer.hasDedicatedSurfaceResources,
                          renderer.estimatedDedicatedGPUBytes == 0
                    else { return false }

                    var cells = [SeyalPreparedCell](
                        repeating: makeCell(),
                        count: geometry.rows * geometry.columns
                    )
                    var generation: UInt64 = 1

                    func lifecycle(measure: Bool) throws -> (update: UInt64, release: UInt64, bytes: Int)? {
                        renderer.setVisible(true)
                        var damage = DamageMask()
                        damage.markAll(rows: geometry.rows)
                        let updateStarted = DispatchTime.now().uptimeNanoseconds
                        let result = try cells.withUnsafeBufferPointer { buffer in
                            try renderer.update(
                                frame: NativePreparedFrame(
                                    cells: buffer,
                                    generation: generation,
                                    rows: geometry.rows,
                                    columns: geometry.columns,
                                    damage: damage
                                ),
                                backingScale: 1,
                                forceFullRebuild: true
                            )
                        }
                        let updateFinished = DispatchTime.now().uptimeNanoseconds
                        guard result == .updated,
                              renderer.hasDedicatedSurfaceResources,
                              renderer.estimatedDedicatedGPUBytes > 0
                        else { return nil }
                        let allocatedBytes = renderer.estimatedDedicatedGPUBytes

                        let releaseStarted = DispatchTime.now().uptimeNanoseconds
                        renderer.setVisible(false)
                        let releaseFinished = DispatchTime.now().uptimeNanoseconds
                        guard !renderer.hasDedicatedSurfaceResources,
                              renderer.estimatedDedicatedGPUBytes == 0
                        else { return nil }
                        generation &+= 1
                        guard measure else { return (0, 0, allocatedBytes) }
                        return (
                            updateFinished - updateStarted,
                            releaseFinished - releaseStarted,
                            allocatedBytes
                        )
                    }

                    for _ in 0..<warmups {
                        guard try lifecycle(measure: false) != nil else { return false }
                    }

                    var updateSamples = [UInt64]()
                    var releaseSamples = [UInt64]()
                    updateSamples.reserveCapacity(measuredCycles)
                    releaseSamples.reserveCapacity(measuredCycles)
                    var maximumDedicatedGPUBytes = 0
                    for cycle in 0..<measuredCycles {
                        let index = cycle % cells.count
                        cells[index].scalar = cells[index].scalar == 0x61 ? 0x62 : 0x61
                        guard let sample = try lifecycle(measure: true) else { return false }
                        updateSamples.append(sample.update)
                        releaseSamples.append(sample.release)
                        maximumDedicatedGPUBytes = max(maximumDedicatedGPUBytes, sample.bytes)
                    }
                    updateSamples.sort()
                    releaseSamples.sort()

                    print(
                        "pass9_renderer_cohort geometry=\(geometry.columns)x\(geometry.rows) cohort=\(cohort) "
                            + "renderer_update_boundary=committed_prepared_state_to_Metal_resources_ready "
                            + "update_p50_us=\(String(format: "%.3f", Double(percentile(updateSamples, 50)) / 1_000.0)) "
                            + "update_p95_us=\(String(format: "%.3f", Double(percentile(updateSamples, 95)) / 1_000.0)) "
                            + "update_p99_us=\(String(format: "%.3f", Double(percentile(updateSamples, 99)) / 1_000.0)) "
                            + "update_max_us=\(String(format: "%.3f", Double(updateSamples.last ?? 0) / 1_000.0)) "
                            + "release_boundary=setVisible_false_to_dedicated_resources_zero "
                            + "release_p50_us=\(String(format: "%.3f", Double(percentile(releaseSamples, 50)) / 1_000.0)) "
                            + "release_p95_us=\(String(format: "%.3f", Double(percentile(releaseSamples, 95)) / 1_000.0)) "
                            + "release_p99_us=\(String(format: "%.3f", Double(percentile(releaseSamples, 99)) / 1_000.0)) "
                            + "release_max_us=\(String(format: "%.3f", Double(releaseSamples.last ?? 0) / 1_000.0)) "
                            + "max_dedicated_gpu_bytes=\(maximumDedicatedGPUBytes) "
                            + "resource_return_every_cycle=true sample_count=\(measuredCycles) performance_claim=false"
                    )
                }
            }
        } catch {
            print("pass9_renderer_calibration_error=\(error)")
            return false
        }
        return true
    }
}