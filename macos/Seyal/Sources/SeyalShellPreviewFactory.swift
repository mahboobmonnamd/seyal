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

struct SeyalShortcutHintPolicy {
    static let intentionalHoldDelay: TimeInterval = 0.30

    static func isCommandOnly(_ flags: NSEvent.ModifierFlags) -> Bool {
        let ignored: NSEvent.ModifierFlags = [.capsLock, .numericPad, .function]
        let normalized = flags
            .intersection(.deviceIndependentFlagsMask)
            .subtracting(ignored)
        return normalized == [.command]
    }
}

@MainActor
final class SeyalShortcutHintOverlay: NSView {
    struct Hint: Equatable {
        let targetAccessibilityID: String
        let text: String
        let id: String
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }

    func present(_ hints: [Hint], in root: NSView) {
        root.layoutSubtreeIfNeeded()
        if superview !== root {
            removeFromSuperview()
            frame = root.bounds
            autoresizingMask = [.width, .height]
            root.addSubview(self, positioned: .above, relativeTo: nil)
        } else {
            frame = root.bounds
        }

        subviews.forEach { $0.removeFromSuperview() }
        setAccessibilityIdentifier("shortcut-hint-overlay")
        isHidden = false

        for hint in hints {
            guard let target = descendant(
                in: root,
                accessibilityID: hint.targetAccessibilityID,
                excluding: self
            ), !target.isHidden else {
                continue
            }
            addHintBadge(hint, target: target, root: root)
        }
    }

    func dismiss() {
        isHidden = true
        subviews.forEach { $0.removeFromSuperview() }
    }

    private func addHintBadge(_ hint: Hint, target: NSView, root: NSView) {
        let targetRect = target.convert(target.bounds, to: root)
        guard targetRect.width > 0, targetRect.height > 0 else { return }

        let label = NSTextField(labelWithString: hint.text)
        label.font = NSFont.monospacedSystemFont(ofSize: 9.5, weight: .semibold)
        label.textColor = SeyalDesignTokens.Palette.textPrimary
        label.alignment = .center
        label.setAccessibilityIdentifier("shortcut-hint.\(hint.id)")
        label.sizeToFit()

        let horizontalPadding: CGFloat = 5
        let verticalPadding: CGFloat = 3
        let badgeSize = NSSize(
            width: ceil(label.frame.width + horizontalPadding * 2),
            height: ceil(label.frame.height + verticalPadding * 2)
        )

        var x = targetRect.maxX - badgeSize.width + 4
        var y = targetRect.maxY - badgeSize.height + 4
        x = min(max(4, x), max(4, bounds.width - badgeSize.width - 4))
        y = min(max(4, y), max(4, bounds.height - badgeSize.height - 4))

        let badge = NSView(frame: NSRect(origin: NSPoint(x: x, y: y), size: badgeSize))
        badge.wantsLayer = true
        badge.layer?.cornerRadius = 5
        badge.layer?.backgroundColor = SeyalDesignTokens.Palette.elevatedBackground.withAlphaComponent(0.97).cgColor
        badge.layer?.borderWidth = 1
        badge.layer?.borderColor = SeyalDesignTokens.Palette.focus.cgColor

        label.frame.origin = NSPoint(x: horizontalPadding, y: verticalPadding)
        badge.addSubview(label)
        addSubview(badge)
    }

    private func descendant(
        in root: NSView,
        accessibilityID: String,
        excluding excluded: NSView
    ) -> NSView? {
        if root !== excluded, root.accessibilityIdentifier() == accessibilityID {
            return root
        }
        for child in root.subviews where child !== excluded {
            if let match = descendant(
                in: child,
                accessibilityID: accessibilityID,
                excluding: excluded
            ) {
                return match
            }
        }
        return nil
    }
}

@MainActor
final class SeyalShortcutHintMonitor {
    private weak var window: NSWindow?
    private let onVisibilityChange: (Bool) -> Void
    private var flagsMonitor: Any?
    private var keyDownMonitor: Any?
    private var appResignObserver: NSObjectProtocol?
    private var windowBecomeKeyObserver: NSObjectProtocol?
    private var windowResignKeyObserver: NSObjectProtocol?
    private var windowCloseObserver: NSObjectProtocol?
    private var pendingShowTimer: DispatchSourceTimer?
    private var pendingGeneration = 0
    private var isVisible = false

    init(window: NSWindow, onVisibilityChange: @escaping (Bool) -> Void) {
        self.window = window
        self.onVisibilityChange = onVisibilityChange
    }

    func start() {
        guard flagsMonitor == nil else { return }

        flagsMonitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            self?.update(from: event.modifierFlags, eventWindow: event.window)
            return event
        }
        keyDownMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
            self?.handleKeyDown(event)
            return event
        }

        appResignObserver = NotificationCenter.default.addObserver(
            forName: NSApplication.didResignActiveNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.cancelPendingAndHide()
            }
        }
        if let window {
            windowBecomeKeyObserver = NotificationCenter.default.addObserver(
                forName: NSWindow.didBecomeKeyNotification,
                object: window,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.update(from: NSEvent.modifierFlags, eventWindow: nil)
                }
            }
            windowResignKeyObserver = NotificationCenter.default.addObserver(
                forName: NSWindow.didResignKeyNotification,
                object: window,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.cancelPendingAndHide()
                }
            }
            windowCloseObserver = NotificationCenter.default.addObserver(
                forName: NSWindow.willCloseNotification,
                object: window,
                queue: .main
            ) { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.stop()
                }
            }
        }
    }

    func stop() {
        if let flagsMonitor {
            NSEvent.removeMonitor(flagsMonitor)
            self.flagsMonitor = nil
        }
        if let keyDownMonitor {
            NSEvent.removeMonitor(keyDownMonitor)
            self.keyDownMonitor = nil
        }
        for observer in [
            appResignObserver,
            windowBecomeKeyObserver,
            windowResignKeyObserver,
            windowCloseObserver,
        ].compactMap({ $0 }) {
            NotificationCenter.default.removeObserver(observer)
        }
        appResignObserver = nil
        windowBecomeKeyObserver = nil
        windowResignKeyObserver = nil
        windowCloseObserver = nil
        cancelPendingAndHide()
    }

    func showImmediatelyForTesting() {
        setVisible(true)
    }

    private func handleKeyDown(_ event: NSEvent) {
        guard isCurrentWindow(eventWindow: event.window) else { return }
        cancelPendingAndHide()
    }

    private func update(from flags: NSEvent.ModifierFlags, eventWindow: NSWindow?) {
        guard isCurrentWindow(eventWindow: eventWindow),
              SeyalShortcutHintPolicy.isCommandOnly(flags) else {
            cancelPendingAndHide()
            return
        }
        queueShow()
    }

    private func isCurrentWindow(eventWindow: NSWindow?) -> Bool {
        guard let window, window.isKeyWindow else { return false }
        if let eventWindow {
            return eventWindow === window
        }
        return NSApp.keyWindow === window
    }

    private func queueShow() {
        guard !isVisible, pendingShowTimer == nil else { return }

        pendingGeneration &+= 1
        let generation = pendingGeneration
        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now() + SeyalShortcutHintPolicy.intentionalHoldDelay)
        timer.setEventHandler { [weak self] in
            Task { @MainActor [weak self] in
                self?.showIfStillEligible(generation: generation)
            }
        }
        pendingShowTimer = timer
        timer.resume()
    }

    private func showIfStillEligible(generation: Int) {
        guard pendingGeneration == generation else { return }
        pendingShowTimer?.cancel()
        pendingShowTimer = nil
        guard isCurrentWindow(eventWindow: nil),
              SeyalShortcutHintPolicy.isCommandOnly(NSEvent.modifierFlags) else {
            return
        }
        setVisible(true)
    }

    private func cancelPendingAndHide() {
        pendingGeneration &+= 1
        pendingShowTimer?.cancel()
        pendingShowTimer = nil
        setVisible(false)
    }

    private func setVisible(_ visible: Bool) {
        guard visible != isVisible else { return }
        isVisible = visible
        onVisibilityChange(visible)
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
    enum CloseTarget: Equatable {
        case pane(String)
        case tab(String)
        case window
    }

    private weak var window: NSWindow?
    private let state: SeyalShellPreviewState
    private let hintOverlay = SeyalShortcutHintOverlay(frame: .zero)
    private var hintMonitor: SeyalShortcutHintMonitor?

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
        installHintMonitor()
    }

    func showShortcutHintsForTesting() {
        hintMonitor?.showImmediatelyForTesting()
    }

    private func installHintMonitor() {
        guard let window, hintMonitor == nil else { return }
        let monitor = SeyalShortcutHintMonitor(window: window) { [weak self] visible in
            guard let self else { return }
            if visible {
                self.presentShortcutHints()
            } else {
                self.hintOverlay.dismiss()
            }
        }
        hintMonitor = monitor
        monitor.start()
    }

    private func presentShortcutHints() {
        guard let root = window?.contentView else { return }
        hintOverlay.present(currentShortcutHints(), in: root)
    }

    private func currentShortcutHints() -> [SeyalShortcutHintOverlay.Hint] {
        var hints: [SeyalShortcutHintOverlay.Hint] = []

        for (index, workspace) in state.workspaces.prefix(9).enumerated() {
            hints.append(.init(
                targetAccessibilityID: "workspace.\(workspace.id)",
                text: "⌃⌘\(index + 1)",
                id: "workspace.\(workspace.id)"
            ))
        }
        for (index, tab) in state.activeWorkspace.tabs.prefix(9).enumerated() {
            hints.append(.init(
                targetAccessibilityID: "tab.\(tab.id)",
                text: "⌘\(index + 1)",
                id: "tab.\(tab.id)"
            ))
        }

        hints.append(contentsOf: [
            .init(targetAccessibilityID: "new-tab", text: "⌘T", id: "new-tab"),
            .init(targetAccessibilityID: "toggle-left-sidebar", text: "⌘0", id: "left-sidebar"),
            .init(targetAccessibilityID: "toggle-inspector", text: "⌥⌘0", id: "inspector"),
            .init(
                targetAccessibilityID: "pane.focus.\(state.activeTab.focusedPaneID)",
                text: "⌘W",
                id: "close-focused-context"
            ),
        ])
        return hints
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
            title: "Close Focused Pane / Tab / Window",
            action: #selector(closeFocusedContext(_:)),
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
    func closeFocusedContext(_ sender: Any?) {
        switch Self.closeTarget(for: state) {
        case let .pane(paneID):
            sendShellAction("closePane:", identifier: paneID)
        case let .tab(tabID):
            sendShellAction("closeTab:", identifier: tabID)
        case .window:
            window?.performClose(nil)
        }
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

    static func closeTarget(for state: SeyalShellPreviewState) -> CloseTarget {
        if state.activeTab.paneCount > 1 {
            return .pane(state.activeTab.focusedPaneID)
        }
        if state.activeWorkspace.tabs.count > 1 {
            return .tab(state.activeTab.id)
        }
        return .window
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
