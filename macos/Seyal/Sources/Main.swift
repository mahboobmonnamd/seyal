import AppKit
import Darwin

@main
enum SeyalMain {
    @MainActor
    static func main() {
        if CommandLine.arguments.contains("--smoke-test") {
            guard MetalSurfaceView.smokeTest() else {
                print("Seyal native smoke test failed.")
                exit(1)
            }
            print("Seyal native Swift/AppKit/Metal smoke test passed.")
            return
        }

        if CommandLine.arguments.contains("--renderer-self-test") {
            guard RendererValidation.deterministicSelfTest(),
                  Pass6RegressionValidation.selfTest(),
                  MetalTerminalRenderer.gpuCompletionFailureRecoverySelfTest(),
                  InteractiveMetalSurfaceView.pass7InputSelfTest()
            else {
                print("Seyal deterministic Metal renderer/input self-test failed.")
                exit(1)
            }
            print("Seyal deterministic Metal renderer/input self-test passed.")
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

        if CommandLine.arguments.contains("--pass7-native-input-benchmark") {
            guard runPass7NativeInputBenchmark() else {
                print("Seyal Pass 7 native input benchmark failed.")
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
        guard window.makeFirstResponder(surface), surface.terminalBridgeIsConnected else {
            window.orderOut(nil)
            return false
        }

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
                + "p50_us=\(String(format: \"%.3f\", p50)) "
                + "p95_us=\(String(format: \"%.3f\", p95)) "
                + "p99_us=\(String(format: \"%.3f\", p99)) "
                + "max_us=\(String(format: \"%.3f\", maximum)) "
                + "appkit_event_boundary=true production_keyDown_route=true "
                + "synthetic_event=true physical_keyboard=false performance_claim=false"
        )
        window.orderOut(nil)
        return true
    }
}
