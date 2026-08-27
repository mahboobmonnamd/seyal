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
                  MetalTerminalRenderer.gpuCompletionFailureRecoverySelfTest()
            else {
                print("Seyal deterministic Metal renderer self-test failed.")
                exit(1)
            }
            print("Seyal deterministic Metal renderer self-test passed.")
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
}
