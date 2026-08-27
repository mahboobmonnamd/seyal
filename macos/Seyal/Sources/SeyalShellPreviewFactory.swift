import AppKit

#if DEBUG
@MainActor
enum SeyalShellPreviewFactory {
    static func make(
        frame: NSRect,
        state: SeyalShellPreviewState? = nil
    ) -> SeyalShellView {
        let resolvedState = state ?? SeyalShellPreviewState.makeDefault(
            includeTestAttention: ProcessInfo.processInfo.environment["SEYAL_UI_TEST_FIXTURES"] == "1"
        )
        let shell = SeyalShellView(frame: frame, state: resolvedState)
        shell.translatesAutoresizingMaskIntoConstraints = true
        shell.autoresizingMask = [.width, .height]
        return shell
    }
}

/// Native macOS command routing for the deterministic shell preview.
///
/// Keyboard shortcuts are expressed as NSMenuItem key equivalents rather than
/// raw keyDown interception. That keeps them discoverable in the menu bar,
/// participates in standard AppKit command handling/accessibility, and avoids
/// stealing non-Command terminal input that must eventually reach the PTY path.
@MainActor
final class SeyalPreviewShortcutController: NSObject {
    private weak var window: NSWindow?
    private let state: SeyalShellPreviewState

    init(window: NSWindow, state: SeyalShellPreviewState) {
        self.window = window
        self.state = state
        super.init()
    }

    func installMenus() {
        let mainMenu = NSMenu(title: "Main Menu")
        mainMenu.addItem(makeApplicationMenu())
        mainMenu.addItem(makeViewMenu())
        mainMenu.addItem(makeWorkspaceMenu())
        mainMenu.addItem(makeTabMenu())
        let windowRoot = makeWindowMenu()
        mainMenu.addItem(windowRoot)

        NSApp.mainMenu = mainMenu
        NSApp.windowsMenu = windowRoot.submenu
    }

    private func makeApplicationMenu() -> NSMenuItem {
        let root = NSMenuItem(title: "Seyal", action: nil, keyEquivalent: "")
        let menu = NSMenu(title: "Seyal")

        let quit = NSMenuItem(
            title: "Quit Seyal",
            action: #selector(NSApplication.terminate(_:)),
            keyEquivalent: "q"
        )
        quit.target = NSApp
        quit.keyEquivalentModifierMask = [.command]
        menu.addItem(quit)
        root.submenu = menu
        return root
    }

    private func makeViewMenu() -> NSMenuItem {
        let root = NSMenuItem(title: "View", action: nil, keyEquivalent: "")
        let menu = NSMenu(title: "View")

        menu.addItem(makeItem(
            title: "Toggle Navigation Sidebar",
            action: #selector(toggleNavigationSidebar(_:)),
            keyEquivalent: "0",
            modifiers: [.command]
        ))
        menu.addItem(makeItem(
            title: "Toggle Inspector",
            action: #selector(toggleInspector(_:)),
            keyEquivalent: "0",
            modifiers: [.command, .option]
        ))

        root.submenu = menu
        return root
    }

    private func makeWorkspaceMenu() -> NSMenuItem {
        let root = NSMenuItem(title: "Workspace", action: nil, keyEquivalent: "")
        let menu = NSMenu(title: "Workspace")

        menu.addItem(makeItem(
            title: "Previous Workspace",
            action: #selector(previousWorkspace(_:)),
            keyEquivalent: "[",
            modifiers: [.command, .control]
        ))
        menu.addItem(makeItem(
            title: "Next Workspace",
            action: #selector(nextWorkspace(_:)),
            keyEquivalent: "]",
            modifiers: [.command, .control]
        ))
        menu.addItem(.separator())

        for index in 0..<9 {
            let title = index < state.workspaces.count
                ? "Workspace \(index + 1) — \(state.workspaces[index].name)"
                : "Workspace \(index + 1)"
            let item = makeItem(
                title: title,
                action: #selector(selectWorkspaceByNumber(_:)),
                keyEquivalent: String(index + 1),
                modifiers: [.command, .control]
            )
            item.tag = index
            item.isEnabled = index < state.workspaces.count
            menu.addItem(item)
        }

        root.submenu = menu
        return root
    }

    private func makeTabMenu() -> NSMenuItem {
        let root = NSMenuItem(title: "Tab", action: nil, keyEquivalent: "")
        let menu = NSMenu(title: "Tab")

        menu.addItem(makeItem(
            title: "New Tab",
            action: #selector(newTab(_:)),
            keyEquivalent: "t",
            modifiers: [.command]
        ))
        menu.addItem(makeItem(
            title: "Close Tab",
            action: #selector(closeTab(_:)),
            keyEquivalent: "w",
            modifiers: [.command]
        ))
        menu.addItem(.separator())
        menu.addItem(makeItem(
            title: "Previous Tab",
            action: #selector(previousTab(_:)),
            keyEquivalent: "[",
            modifiers: [.command, .shift]
        ))
        menu.addItem(makeItem(
            title: "Next Tab",
            action: #selector(nextTab(_:)),
            keyEquivalent: "]",
            modifiers: [.command, .shift]
        ))
        menu.addItem(.separator())

        for index in 0..<9 {
            let item = makeItem(
                title: "Tab \(index + 1)",
                action: #selector(selectTabByNumber(_:)),
                keyEquivalent: String(index + 1),
                modifiers: [.command]
            )
            item.tag = index
            menu.addItem(item)
        }

        root.submenu = menu
        return root
    }

    private func makeWindowMenu() -> NSMenuItem {
        let root = NSMenuItem(title: "Window", action: nil, keyEquivalent: "")
        let menu = NSMenu(title: "Window")

        menu.addItem(makeItem(
            title: "Previous Window",
            action: #selector(previousWindow(_:)),
            keyEquivalent: "`",
            modifiers: [.command, .shift]
        ))
        menu.addItem(makeItem(
            title: "Next Window",
            action: #selector(nextWindow(_:)),
            keyEquivalent: "`",
            modifiers: [.command]
        ))
        menu.addItem(.separator())

        for index in 0..<9 {
            let item = makeItem(
                title: "Window \(index + 1)",
                action: #selector(selectWindowByNumber(_:)),
                keyEquivalent: String(index + 1),
                modifiers: [.command, .option]
            )
            item.tag = index
            menu.addItem(item)
        }

        root.submenu = menu
        return root
    }

    private func makeItem(
        title: String,
        action: Selector,
        keyEquivalent: String,
        modifiers: NSEvent.ModifierFlags
    ) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: keyEquivalent)
        item.target = self
        item.keyEquivalentModifierMask = modifiers
        return item
    }

    @objc
    func previousWorkspace(_ sender: Any?) {
        selectWorkspace(offset: -1)
    }

    @objc
    func nextWorkspace(_ sender: Any?) {
        selectWorkspace(offset: 1)
    }

    @objc
    func selectWorkspaceByNumber(_ sender: NSMenuItem) {
        selectWorkspace(index: sender.tag)
    }

    @objc
    func previousTab(_ sender: Any?) {
        selectTab(offset: -1)
    }

    @objc
    func nextTab(_ sender: Any?) {
        selectTab(offset: 1)
    }

    @objc
    func selectTabByNumber(_ sender: NSMenuItem) {
        selectTab(index: sender.tag)
    }

    @objc
    func newTab(_ sender: Any?) {
        sendShellAction("createTab:")
    }

    @objc
    func closeTab(_ sender: Any?) {
        sendShellAction("closeTab:", identifier: state.activeTab.id)
    }

    @objc
    func toggleNavigationSidebar(_ sender: Any?) {
        sendShellAction("toggleLeftSidebar:")
    }

    @objc
    func toggleInspector(_ sender: Any?) {
        sendShellAction("toggleInspector:")
    }

    @objc
    func previousWindow(_ sender: Any?) {
        cycleWindow(offset: -1)
    }

    @objc
    func nextWindow(_ sender: Any?) {
        cycleWindow(offset: 1)
    }

    @objc
    func selectWindowByNumber(_ sender: NSMenuItem) {
        selectWindow(index: sender.tag)
    }

    private func selectWorkspace(offset: Int) {
        guard !state.workspaces.isEmpty,
              let current = state.workspaces.firstIndex(where: { $0.id == state.activeWorkspaceID }) else {
            return
        }
        let target = Self.wrappedIndex(current: current, count: state.workspaces.count, offset: offset)
        selectWorkspace(index: target)
    }

    private func selectWorkspace(index: Int) {
        guard state.workspaces.indices.contains(index) else { return }
        sendShellAction("selectWorkspace:", identifier: state.workspaces[index].id)
    }

    private func selectTab(offset: Int) {
        let tabs = state.activeWorkspace.tabs
        guard !tabs.isEmpty,
              let current = tabs.firstIndex(where: { $0.id == state.activeWorkspace.activeTabID }) else {
            return
        }
        let target = Self.wrappedIndex(current: current, count: tabs.count, offset: offset)
        selectTab(index: target)
    }

    private func selectTab(index: Int) {
        let tabs = state.activeWorkspace.tabs
        guard tabs.indices.contains(index) else { return }
        sendShellAction("selectTab:", identifier: tabs[index].id)
    }

    private func sendShellAction(_ selectorName: String, identifier: String? = nil) {
        guard let shell = window?.contentView else { return }
        let selector = NSSelectorFromString(selectorName)
        guard shell.responds(to: selector) else {
            NSSound.beep()
            return
        }

        let sender = NSButton()
        if let identifier {
            sender.identifier = NSUserInterfaceItemIdentifier(identifier)
        }
        NSApp.sendAction(selector, to: shell, from: sender)
    }

    private func cycleWindow(offset: Int) {
        let windows = switchableWindows()
        guard windows.count > 1 else { return }
        let currentWindow = NSApp.keyWindow ?? window
        let currentIndex = windows.firstIndex { $0 === currentWindow } ?? 0
        let target = Self.wrappedIndex(current: currentIndex, count: windows.count, offset: offset)
        activateWindow(windows[target])
    }

    private func selectWindow(index: Int) {
        let windows = switchableWindows()
        guard windows.indices.contains(index) else { return }
        activateWindow(windows[index])
    }

    private func switchableWindows() -> [NSWindow] {
        NSApp.windows
            .filter { ($0.isVisible || $0.isMiniaturized) && $0.styleMask.contains(.titled) }
            .sorted { $0.windowNumber < $1.windowNumber }
    }

    private func activateWindow(_ target: NSWindow) {
        if target.isMiniaturized {
            target.deminiaturize(nil)
        }
        target.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    static func wrappedIndex(current: Int, count: Int, offset: Int) -> Int {
        precondition(count > 0)
        let value = (current + offset) % count
        return value >= 0 ? value : value + count
    }
}
#endif
