import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private static let buildConfigurationKey = "SeyalBuildConfiguration"

    private var window: NSWindow?

    static func shouldUseShellPreview(
        arguments: [String],
        environment: [String: String],
        buildConfiguration: String?
    ) -> Bool {
        guard buildConfiguration == "Debug" else {
            return false
        }

        return arguments.contains("--ui-shell-preview")
            || environment["SEYAL_UI_SHELL_PREVIEW"] == "1"
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let buildConfiguration = Bundle.main.object(
            forInfoDictionaryKey: Self.buildConfigurationKey
        ) as? String
        let useShellPreview = Self.shouldUseShellPreview(
            arguments: CommandLine.arguments,
            environment: ProcessInfo.processInfo.environment,
            buildConfiguration: buildConfiguration
        )

        let contentRect = useShellPreview
            ? NSRect(x: 0, y: 0, width: 1280, height: 800)
            : NSRect(x: 0, y: 0, width: 960, height: 600)

        let window = NSWindow(
            contentRect: contentRect,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )

        if useShellPreview {
            window.title = "Seyal — UI Shell Preview"
            window.contentView = SeyalShellPreviewFactory.make(frame: contentRect)
            window.minSize = NSSize(width: 980, height: 650)
        } else {
            window.title = "Seyal"
            let surface = InteractiveMetalSurfaceView(frame: contentRect)
            window.contentView = surface
            window.makeFirstResponder(surface)
        }
#else
        window.title = "Seyal"
        let surface = InteractiveMetalSurfaceView(frame: contentRect)
        window.contentView = surface
        window.makeFirstResponder(surface)
#endif

        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}
