import AppKit

/// Thin AppKit projection of Rust shell/chrome/composer/recovery. No writable product model.
@MainActor
final class ProductChromeHostView: NSView {
    let pane: ThinPaneHostView
    private let material = NSVisualEffectView()
    private let left = NSStackView()
    private let inspector = NSStackView()
    private let attention = NSStackView()
    private let blocks = NSStackView()
    private let composer: ComposerBridgeView
    private let workspacesButton = NSButton(title: "Workspaces", target: nil, action: nil)
    private let tabsButton = NSButton(title: "Tabs", target: nil, action: nil)
    private let recoveryLabel = NSTextField(labelWithString: "")
    private let leftItems = NSStackView()
    private var recoveryTimer: Timer?

    override init(frame frameRect: NSRect) {
        pane = ThinPaneHostView(frame: frameRect)
        composer = ComposerBridgeView(appHandle: pane.appHandle)
        super.init(frame: frameRect)
        translatesAutoresizingMaskIntoConstraints = false
        setAccessibilityIdentifier("seyal-product-chrome")
        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        wantsLayer = true

        material.translatesAutoresizingMaskIntoConstraints = false
        material.blendingMode = .behindWindow
        material.state = .active
        addSubview(material)

        left.orientation = .vertical
        left.alignment = .leading
        left.translatesAutoresizingMaskIntoConstraints = false
        leftItems.orientation = .vertical
        leftItems.alignment = .leading
        leftItems.translatesAutoresizingMaskIntoConstraints = false
        workspacesButton.setAccessibilityIdentifier("seyal-left-workspaces")
        tabsButton.setAccessibilityIdentifier("seyal-left-tabs")
        workspacesButton.target = self
        workspacesButton.action = #selector(showWorkspaces)
        tabsButton.target = self
        tabsButton.action = #selector(showTabs)
        left.addArrangedSubview(workspacesButton)
        left.addArrangedSubview(tabsButton)
        left.addArrangedSubview(leftItems)

        inspector.orientation = .vertical
        inspector.alignment = .leading
        inspector.translatesAutoresizingMaskIntoConstraints = false
        inspector.setAccessibilityIdentifier("seyal-inspector")
        attention.orientation = .vertical
        attention.alignment = .leading
        attention.setAccessibilityIdentifier("seyal-attention")
        blocks.orientation = .vertical
        blocks.alignment = .leading
        blocks.translatesAutoresizingMaskIntoConstraints = false
        blocks.setAccessibilityIdentifier("seyal-blocks")
        composer.setAccessibilityIdentifier("seyal-composer")
        recoveryLabel.setAccessibilityIdentifier("seyal-recovery")

        let inspectorColumn = NSStackView(views: [inspector, attention, recoveryLabel])
        inspectorColumn.orientation = .vertical
        inspectorColumn.alignment = .leading
        inspectorColumn.translatesAutoresizingMaskIntoConstraints = false

        let center = NSStackView(views: [blocks, pane, composer])
        center.orientation = .vertical
        center.translatesAutoresizingMaskIntoConstraints = false
        addSubview(left)
        addSubview(center)
        addSubview(inspectorColumn)

        NSLayoutConstraint.activate([
            material.leadingAnchor.constraint(equalTo: leadingAnchor),
            material.trailingAnchor.constraint(equalTo: trailingAnchor),
            material.topAnchor.constraint(equalTo: topAnchor),
            material.bottomAnchor.constraint(equalTo: bottomAnchor),
            left.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            left.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            left.widthAnchor.constraint(equalToConstant: 160),
            center.leadingAnchor.constraint(equalTo: left.trailingAnchor, constant: 8),
            center.trailingAnchor.constraint(equalTo: inspectorColumn.leadingAnchor, constant: -8),
            center.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            center.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -8),
            inspectorColumn.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            inspectorColumn.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            inspectorColumn.widthAnchor.constraint(equalToConstant: 220),
            pane.heightAnchor.constraint(greaterThanOrEqualToConstant: 240),
        ])

        pane.onProductChanged = { [weak self] in
            self?.reconcileChrome()
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("ProductChromeHostView is programmatic")
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        applyTheme()
    }

    var inputSurface: InteractiveMetalSurfaceView { pane.inputSurface }

    func activateAfterWindowPresentation() {
        beginRecovery()
        pane.activateAfterWindowPresentation()
        reconcileChrome()
        applyTheme()
    }

    func requestQuit() { pane.requestQuit() }
    func detachForTermination() { pane.detachForTermination() }

    func reconcileChrome() {
        let chrome = seyal_app_chrome(pane.appHandle)
        let shell = seyal_app_shell(pane.appHandle)
        let snapshot = seyal_app_snapshot(pane.appHandle)
        workspacesButton.state = chrome.left_panel == 0 ? .on : .off
        tabsButton.state = chrome.left_panel == 1 ? .on : .off
        rebuildLeft(shell: shell, leftPanel: chrome.left_panel)
        rebuildInspector(chrome)
        rebuildBlocks()
        recoveryLabel.stringValue =
            "Recovery \(snapshot.recovery_stage) · attempts \(snapshot.recovery_attempts)"
        composer.reconcile()
        driveRecovery()
    }

    private func rebuildLeft(shell: SeyalAppShell, leftPanel: UInt16) {
        leftItems.arrangedSubviews.forEach { $0.removeFromSuperview() }
        if leftPanel == 0 {
            for index in 0..<Int(shell.workspace_count) {
                let row = seyal_app_shell_row(pane.appHandle, UInt16(SEYAL_APP_ROW_WORKSPACE), UInt32(index))
                leftItems.addArrangedSubview(
                    rowButton(
                        title: copyUTF8(row.title, row.title_len) ?? "Workspace",
                        identifier: "seyal-workspace-\(index)",
                        selected: row.flags & UInt16(SEYAL_APP_ROW_SELECTED) != 0,
                        action: #selector(selectWorkspace(_:)),
                        kind: UInt16(SEYAL_APP_ACTION_SELECT_WORKSPACE.rawValue),
                        idLo: row.id_lo,
                        idHi: row.id_hi
                    )
                )
            }
        } else {
            for index in 0..<Int(shell.tab_count) {
                let row = seyal_app_shell_row(pane.appHandle, UInt16(SEYAL_APP_ROW_TAB), UInt32(index))
                leftItems.addArrangedSubview(
                    rowButton(
                        title: copyUTF8(row.title, row.title_len) ?? "Tab",
                        identifier: "seyal-tab-\(index)",
                        selected: row.flags & UInt16(SEYAL_APP_ROW_SELECTED) != 0,
                        action: #selector(selectTab(_:)),
                        kind: UInt16(SEYAL_APP_ACTION_SELECT_TAB.rawValue),
                        idLo: row.id_lo,
                        idHi: row.id_hi
                    )
                )
            }
        }
        for index in 0..<Int(shell.pane_count) {
            let row = seyal_app_shell_row(pane.appHandle, UInt16(SEYAL_APP_ROW_PANE), UInt32(index))
            leftItems.addArrangedSubview(
                rowButton(
                    title: copyUTF8(row.title, row.title_len) ?? "Pane",
                    identifier: "seyal-pane-\(index)",
                    selected: row.flags & UInt16(SEYAL_APP_ROW_SELECTED) != 0,
                    action: #selector(focusPane(_:)),
                    kind: UInt16(SEYAL_APP_ACTION_FOCUS_PANE.rawValue),
                    idLo: row.id_lo,
                    idHi: row.id_hi
                )
            )
        }
    }

    private func rebuildInspector(_ chrome: SeyalAppChrome) {
        inspector.arrangedSubviews.forEach { $0.removeFromSuperview() }
        attention.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for index in 0..<Int(chrome.inspector_row_count) {
            let row = seyal_app_chrome_row(pane.appHandle, UInt16(SEYAL_APP_ROW_INSPECTOR), UInt32(index))
            let title = copyUTF8(row.title, row.title_len) ?? ""
            let value = copyUTF8(row.detail, row.detail_len) ?? ""
            let label = NSTextField(labelWithString: "\(title): \(value)")
            label.maximumNumberOfLines = 2
            inspector.addArrangedSubview(label)
        }
        for index in 0..<Int(chrome.attention_count) {
            let row = seyal_app_chrome_row(pane.appHandle, UInt16(SEYAL_APP_ROW_ATTENTION), UInt32(index))
            let identity = copyUTF8(row.title, row.title_len) ?? ""
            let title = copyUTF8(row.detail, row.detail_len) ?? identity
            let button = NSButton(title: title, target: self, action: #selector(openAttention(_:)))
            button.setAccessibilityIdentifier("seyal-attention-\(index)")
            button.identifier = NSUserInterfaceItemIdentifier(identity)
            attention.addArrangedSubview(button)
        }
        for index in 0..<Int(chrome.agent_count) {
            let row = seyal_app_chrome_row(pane.appHandle, UInt16(SEYAL_APP_ROW_AGENT), UInt32(index))
            let identity = copyUTF8(row.title, row.title_len) ?? ""
            let name = copyUTF8(row.detail, row.detail_len) ?? identity
            let button = NSButton(title: name, target: self, action: #selector(selectAgent(_:)))
            button.setAccessibilityIdentifier("seyal-agent-\(index)")
            button.identifier = NSUserInterfaceItemIdentifier(identity)
            button.state = row.flags & UInt16(SEYAL_APP_ROW_SELECTED) != 0 ? .on : .off
            attention.addArrangedSubview(button)
        }
    }

    private func rebuildBlocks() {
        blocks.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let composerSnap = seyal_app_composer(pane.appHandle)
        for index in 0..<Int(composerSnap.block_count) {
            let row = seyal_app_block_row(pane.appHandle, UInt32(index))
            let command = copyUTF8(row.title, row.title_len) ?? ""
            let state = copyUTF8(row.detail, row.detail_len) ?? ""
            let label = NSTextField(labelWithString: "\(command) · \(state)")
            label.setAccessibilityIdentifier("seyal-block-\(index)")
            blocks.addArrangedSubview(label)
        }
    }

    private func applyTheme() {
        NativeThemeRealization.apply(
            to: self,
            material: material,
            appearance: effectiveAppearance
        )
    }

    private func beginRecovery() {
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_BEGIN_RECOVERY.rawValue)
        action.target_pty_generation = UInt64(Date().timeIntervalSince1970 * 1000)
        _ = seyal_app_apply(pane.appHandle, &action)
        driveRecovery()
    }

    private func driveRecovery() {
        let snapshot = seyal_app_snapshot(pane.appHandle)
        switch snapshot.recovery_effect {
        case UInt32(SEYAL_APP_RECOVERY_EFFECT_PERFORM_ATTEMPT.rawValue):
            completeRecovery(connected: pane.inputSurface.terminalBridgeIsConnected)
        case UInt32(SEYAL_APP_RECOVERY_EFFECT_SCHEDULE.rawValue):
            let delayMs = max(seyal_app_recovery_param(pane.appHandle), 10)
            recoveryTimer?.invalidate()
            let generation = snapshot.recovery_generation
            recoveryTimer = Timer.scheduledTimer(
                withTimeInterval: TimeInterval(delayMs) / 1000,
                repeats: false
            ) { [weak self] _ in
                DispatchQueue.main.async {
                    self?.fireRecovery(generation: generation)
                }
            }
        case UInt32(SEYAL_APP_RECOVERY_EFFECT_LAUNCH_HELPER.rawValue):
            _ = BundledRuntimeLauncher().launch()
        case UInt32(SEYAL_APP_RECOVERY_EFFECT_DISPOSE_HANDLE.rawValue):
            seyal_bridge_disconnect_handle(seyal_app_recovery_param(pane.appHandle))
        default:
            break
        }
        ackRecovery()
    }

    private func completeRecovery(connected: Bool, helperMissing: Bool = false) {
        let snapshot = seyal_app_snapshot(pane.appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_COMPLETE_RECOVERY.rawValue)
        action.target_execution_lo = snapshot.recovery_generation
        action.target_pty_generation = UInt64(Date().timeIntervalSince1970 * 1000)
        if connected {
            action.reserved = UInt32(SEYAL_APP_RECOVERY_CONNECTED.rawValue)
        } else if helperMissing {
            action.reserved = UInt32(SEYAL_APP_RECOVERY_ENDPOINT_MISSING.rawValue)
                | (UInt32(SEYAL_APP_RECOVERY_LAUNCH_HELPER_MISSING.rawValue) << 8)
        } else {
            action.reserved = UInt32(SEYAL_APP_RECOVERY_ENDPOINT_MISSING.rawValue)
                | (UInt32(SEYAL_APP_RECOVERY_LAUNCH_STARTED.rawValue) << 8)
        }
        _ = seyal_app_apply(pane.appHandle, &action)
    }

    private func fireRecovery(generation: UInt64) {
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_FIRE_RECOVERY.rawValue)
        action.target_execution_lo = generation
        action.target_pty_generation = UInt64(Date().timeIntervalSince1970 * 1000)
        _ = seyal_app_apply(pane.appHandle, &action)
        driveRecovery()
    }

    private func ackRecovery() {
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_ACK_RECOVERY.rawValue)
        _ = seyal_app_apply(pane.appHandle, &action)
    }

    @objc private func showWorkspaces() {
        applyChromeKind(UInt16(SEYAL_APP_ACTION_SET_LEFT_PANEL.rawValue), reserved: 0)
    }

    @objc private func showTabs() {
        applyChromeKind(UInt16(SEYAL_APP_ACTION_SET_LEFT_PANEL.rawValue), reserved: 1)
    }

    @objc private func selectWorkspace(_ sender: NSButton) {
        applyIdentity(UInt16(SEYAL_APP_ACTION_SELECT_WORKSPACE.rawValue), button: sender)
    }

    @objc private func selectTab(_ sender: NSButton) {
        applyIdentity(UInt16(SEYAL_APP_ACTION_SELECT_TAB.rawValue), button: sender)
    }

    @objc private func focusPane(_ sender: NSButton) {
        applyIdentity(UInt16(SEYAL_APP_ACTION_FOCUS_PANE.rawValue), button: sender)
    }

    @objc private func openAttention(_ sender: NSButton) {
        applyPayload(UInt16(SEYAL_APP_ACTION_OPEN_ATTENTION.rawValue), text: sender.identifier?.rawValue ?? "")
    }

    @objc private func selectAgent(_ sender: NSButton) {
        applyPayload(UInt16(SEYAL_APP_ACTION_SELECT_AGENT.rawValue), text: sender.identifier?.rawValue ?? "")
    }

    private func applyChromeKind(_ kind: UInt16, reserved: UInt32) {
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = kind
        action.reserved = reserved
        _ = seyal_app_apply(pane.appHandle, &action)
        reconcileChrome()
    }

    private func applyIdentity(_ kind: UInt16, button: NSButton) {
        guard let tagged = button as? IdentityButton else { return }
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = kind
        action.target_execution_lo = tagged.idLo
        action.target_execution_hi = tagged.idHi
        _ = seyal_app_apply(pane.appHandle, &action)
        reconcileChrome()
    }

    private func applyPayload(_ kind: UInt16, text: String) {
        let snapshot = seyal_app_snapshot(pane.appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = kind
        action.fence_pane_lo = snapshot.pane_lo
        action.fence_pane_hi = snapshot.pane_hi
        action.fence_epoch = snapshot.epoch
        let utf8 = Array(text.utf8)
        utf8.withUnsafeBufferPointer { buffer in
            action.payload = buffer.baseAddress
            action.payload_len = UInt32(buffer.count)
            _ = seyal_app_apply(pane.appHandle, &action)
        }
        reconcileChrome()
    }

    private func rowButton(
        title: String,
        identifier: String,
        selected: Bool,
        action: Selector,
        kind: UInt16,
        idLo: UInt64,
        idHi: UInt64
    ) -> IdentityButton {
        let button = IdentityButton(title: title, target: self, action: action)
        button.idLo = idLo
        button.idHi = idHi
        button.kind = kind
        button.setAccessibilityIdentifier(identifier)
        button.state = selected ? .on : .off
        return button
    }
}

private final class IdentityButton: NSButton {
    var kind: UInt16 = 0
    var idLo: UInt64 = 0
    var idHi: UInt64 = 0
}

private func copyUTF8(_ pointer: UnsafePointer<UInt8>?, _ length: UInt32) -> String? {
    guard length > 0, let pointer else { return nil }
    return String(decoding: UnsafeBufferPointer(start: pointer, count: Int(length)), as: UTF8.self)
}
