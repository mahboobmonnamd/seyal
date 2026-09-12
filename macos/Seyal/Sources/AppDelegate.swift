import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private static let buildConfigurationKey = "SeyalBuildConfiguration"

    private var window: NSWindow?
    private var appearance: SeyalAppearanceController?

    /// Deprecated Debug preview flag. Leftover XCTest may still query this
    /// until #884. The supported launch path never installs `SeyalShellState`.
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
        let contentRect = NSRect(x: 0, y: 0, width: 960, height: 600)
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

        // M001.1 / #890: the rejected Swift product shell is not a supported
        // headed product path. This window is a native-glue harness only
        // (Metal / IME / helper launch). The new thin host is #883.
        window.appearance = snapshot.nsAppearance
        window.backgroundColor = snapshot.colors.ns(.container)
        window.title = "Seyal"
        window.contentView = SeyalThinHostView(
            frame: contentRect,
            terminalFont: snapshot.terminalFont
        )
        Self.installProductionApplicationMenu()
        appearance.onChange = { [weak self] next in
            guard let window = self?.window else { return }
            window.appearance = next.nsAppearance
            window.backgroundColor = next.colors.ns(.container)
        }
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window
        window.contentView?.layoutSubtreeIfNeeded()
    }

    func applicationWillTerminate(_ notification: Notification) {}

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        .terminateNow
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    private static func installProductionApplicationMenu() {
        NSApp.mainMenu = makeProductionApplicationMenu()
    }

    /// Shared with component tests so keyboard coverage cannot silently test a
    /// hand-written menu that differs from the production responder chain.
    static func makeProductionApplicationMenu() -> NSMenu {
        let appMenu = NSMenu(title: "Seyal")
        let quit = NSMenuItem(
            title: "Quit Seyal",
            action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q"
        )
        quit.keyEquivalentModifierMask = [.command]
        appMenu.addItem(quit)

        let appItem = NSMenuItem()
        appItem.submenu = appMenu

        // AppKit routes standard editing shortcuts through the main menu and
        // then down the responder chain. A quit-only menu leaves an otherwise
        // functional NSTextView unable to receive Command-C/V in production.
        let editMenu = NSMenu(title: "Edit")
        let copy = NSMenuItem(
            title: "Copy",
            action: #selector(NSText.copy(_:)),
            keyEquivalent: "c"
        )
        copy.keyEquivalentModifierMask = [.command]
        editMenu.addItem(copy)
        let paste = NSMenuItem(
            title: "Paste",
            action: #selector(NSText.paste(_:)),
            keyEquivalent: "v"
        )
        paste.keyEquivalentModifierMask = [.command]
        editMenu.addItem(paste)
        editMenu.addItem(.separator())
        let selectAll = NSMenuItem(
            title: "Select All",
            action: #selector(NSText.selectAll(_:)),
            keyEquivalent: "a"
        )
        selectAll.keyEquivalentModifierMask = [.command]
        editMenu.addItem(selectAll)

        let editItem = NSMenuItem()
        editItem.submenu = editMenu
        let mainMenu = NSMenu(title: "Main Menu")
        mainMenu.addItem(appItem)
        mainMenu.addItem(editItem)
        return mainMenu
    }
}
