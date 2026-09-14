import AppKit
import Darwin

@main
enum SeyalMain {
    static func main() {
        if CommandLine.arguments.contains("--renderer-benchmark") {
            let passed = RendererValidation.runBenchmark()
            Darwin.exit(passed ? 0 : 1)
        }
        let app = NSApplication.shared
        app.setActivationPolicy(.regular)
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
    }
}
