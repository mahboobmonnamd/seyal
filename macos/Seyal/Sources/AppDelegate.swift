import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow?
    private var host: ProductChromeHostView?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let host = ProductChromeHostView(frame: NSRect(x: 0, y: 0, width: 1280, height: 800))
        self.host = host

        let window = NSWindow(
            contentRect: host.bounds,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "Seyal"
        window.minSize = NSSize(width: 960, height: 640)
        window.appearance = NSAppearance(named: .darkAqua)
        window.contentView = host
        window.center()
        window.makeKeyAndOrderFront(nil)
        self.window = window

        installMenus()
        host.activateAfterWindowPresentation()
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        host?.requestQuit()
        host?.detachForTermination()
        return .terminateNow
    }

    private func installMenus() {
        let mainMenu = NSMenu()
        let appItem = NSMenuItem()
        mainMenu.addItem(appItem)
        let appMenu = NSMenu()
        appMenu.addItem(
            withTitle: "Quit Seyal",
            action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q"
        )
        appItem.submenu = appMenu

        let editItem = NSMenuItem()
        mainMenu.addItem(editItem)
        let editMenu = NSMenu(title: "Edit")
        editMenu.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editItem.submenu = editMenu

        let viewItem = NSMenuItem()
        mainMenu.addItem(viewItem)
        let viewMenu = NSMenu(title: "View")
        viewMenu.addItem(withTitle: "Toggle Left Panel", action: #selector(ProductChromeHostView.toggleLeftPanel), keyEquivalent: "0")
        let inspectorItem = viewMenu.addItem(
            withTitle: "Toggle Inspector",
            action: #selector(ProductChromeHostView.toggleInspector),
            keyEquivalent: "0"
        )
        inspectorItem.keyEquivalentModifierMask = [.command, .option]
        viewMenu.addItem(withTitle: "New Tab", action: #selector(ProductChromeHostView.createTab), keyEquivalent: "t")
        viewMenu.addItem(withTitle: "Split Right", action: #selector(ProductChromeHostView.splitRight), keyEquivalent: "d")
        let splitDown = viewMenu.addItem(
            withTitle: "Split Down",
            action: #selector(ProductChromeHostView.splitDown),
            keyEquivalent: "d"
        )
        splitDown.keyEquivalentModifierMask = [.command, .shift]
        viewItem.submenu = viewMenu
        NSApp.mainMenu = mainMenu
    }
}
