import AppKit

/// Native window/menu host only. Rejected Swift product shell is gone (#890).
/// This launch path is not a headed Seyal product. #883 writes the thin host.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let contentRect = NSRect(x: 0, y: 0, width: 720, height: 240)
        let window = NSWindow(
            contentRect: contentRect,
            styleMask: [.titled, .closable, .miniaturizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Seyal"
        window.backgroundColor = .windowBackgroundColor

        let label = NSTextField(wrappingLabelWithString: """
            Native glue harness only.

            The rejected Swift product shell (Workspace/Tab/Pane, composer, Blocks, chrome) \
            is not on the supported path. Metal/IME/helper launch remain for #883. \
            This window is not a headed Seyal product.
            """)
        label.translatesAutoresizingMaskIntoConstraints = false
        label.setAccessibilityIdentifier("seyal-native-glue-harness")
        label.alignment = .left

        let content = NSView(frame: contentRect)
        content.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 24),
            label.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -24),
            label.topAnchor.constraint(equalTo: content.topAnchor, constant: 24),
            label.bottomAnchor.constraint(lessThanOrEqualTo: content.bottomAnchor, constant: -24),
        ])
        window.contentView = content
        Self.installProductionApplicationMenu()
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

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

    private static func installProductionApplicationMenu() {
        NSApp.mainMenu = makeProductionApplicationMenu()
    }
}
