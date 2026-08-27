import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private static let buildConfigurationKey = "SeyalBuildConfiguration"

    private var window: NSWindow?
    #if DEBUG
    private var previewShortcutController: SeyalPreviewShortcutController?
    #endif

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
        let environment = ProcessInfo.processInfo.environment
        let useShellPreview = Self.shouldUseShellPreview(
            arguments: CommandLine.arguments,
            environment: environment,
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
            #if DEBUG
            // The frozen reference is a deliberate dark workspace independent of
            // the developer's current macOS appearance. Production theme selection
            // will replace this preview-only choice behind a real theme model.
            window.appearance = NSAppearance(named: .darkAqua)
            window.backgroundColor = SeyalDesignTokens.Palette.windowBackground
            window.title = "Seyal — UI Shell Preview"

            let previewState = SeyalShellPreviewState.makeDefault(
                includeTestAttention: environment["SEYAL_UI_TEST_FIXTURES"] == "1"
            )
            window.contentView = SeyalShellPreviewFactory.make(
                frame: contentRect,
                state: previewState
            )
            window.minSize = NSSize(width: 1080, height: 680)

            let shortcuts = SeyalPreviewShortcutController(window: window, state: previewState)
            shortcuts.installMenus()
            previewShortcutController = shortcuts
            #else
            // shouldUseShellPreview is false for non-Debug builds. Keep this branch
            // self-contained so Release compilation never depends on preview types.
            window.title = "Seyal"
            window.contentView = MetalSurfaceView(frame: contentRect)
            #endif
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

        #if DEBUG
        if useShellPreview, environment["SEYAL_UI_TEST_FORCE_SHORTCUT_HINTS"] == "1" {
            previewShortcutController?.showShortcutHintsForTesting()
        }
        #endif
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}
