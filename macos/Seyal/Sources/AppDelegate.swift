import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private static let buildConfigurationKey = "SeyalBuildConfiguration"

    private var window: NSWindow?
    private var appearance: SeyalAppearanceController?
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

        let previewWidth: CGFloat
        if useShellPreview {
            // The preview must fit the smallest hosted macOS display while still
            // satisfying the frozen shell's minimum horizontal geometry.
            let availableWidth = NSScreen.main?.visibleFrame.width ?? 1280
            previewWidth = min(1280, max(1050, availableWidth - 32))
        } else {
            previewWidth = 960
        }
        let contentRect = useShellPreview
            ? NSRect(x: 0, y: 0, width: previewWidth, height: 800)
            : NSRect(x: 0, y: 0, width: 960, height: 600)

        let window = NSWindow(
            contentRect: contentRect,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )

        let loaded = SeyalUIConfiguration.loadFromDisk()
        let appearance = SeyalAppearanceController(
            settings: loaded.settings,
            diagnostics: loaded.diagnostics
        )
        self.appearance = appearance
        let snapshot = appearance.snapshot

        if useShellPreview {
            #if DEBUG
            window.appearance = snapshot.nsAppearance
            window.backgroundColor = snapshot.colors.ns(.container)
            window.title = "Seyal — UI Shell Preview"

            let previewState = SeyalShellState.makePreview(
                includeTestAttention: environment["SEYAL_UI_TEST_FIXTURES"] == "1"
            )
            let shell = SeyalShellPreviewFactory.make(
                frame: contentRect,
                state: previewState
            )
            shell.applyVisualConfiguration(snapshot)
            window.contentView = shell
            window.minSize = NSSize(width: 1050, height: 680)

            let shortcuts = SeyalPreviewShortcutController(window: window, state: previewState)
            shortcuts.installMenus()
            previewShortcutController = shortcuts
            #else
            window.title = "Seyal"
            window.contentView = MetalSurfaceView(
                frame: contentRect,
                paneID: "unbound",
                terminalFont: snapshot.terminalFont
            )
            #endif
        } else {
            window.appearance = snapshot.nsAppearance
            window.backgroundColor = snapshot.colors.ns(.container)
            window.title = "Seyal"
            window.contentView = SeyalShellProductionFactory.make(
                frame: contentRect,
                visual: snapshot
            )
        }
        appearance.onChange = { [weak self] next in
            guard let window = self?.window else { return }
            window.appearance = next.nsAppearance
            window.backgroundColor = next.colors.ns(.container)
            (window.contentView as? SeyalShellView)?.applyVisualConfiguration(next)
        }
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window

        #if DEBUG
        if useShellPreview, environment["SEYAL_UI_TEST_FORCE_SHORTCUT_HINTS"] == "1" {
            // The preview hierarchy is built synchronously above. Present the
            // test-only overlay from that same authoritative layout instead of
            // racing a fixed-delay callback against hosted-runner startup.
            window.contentView?.layoutSubtreeIfNeeded()
            window.displayIfNeeded()
            previewShortcutController?.showShortcutHintsForTesting()
        }
        #endif
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}
