import AppKit
import Darwin

@main
enum SeyalMain {
    @MainActor
    static func main() {
        if CommandLine.arguments.contains("--smoke-test") {
            guard MetalSurfaceView.smokeTest() else {
                fputs("Seyal native smoke test failed\n", stderr)
                exit(1)
            }
            print("Seyal native Swift/AppKit/Metal smoke test passed.")
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
