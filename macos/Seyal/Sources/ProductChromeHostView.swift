import AppKit

/// Thin AppKit projection of Rust shell/chrome/composer/recovery. No writable product model.
@MainActor
final class ProductChromeHostView: NSView {
    let pane: ThinPaneHostView
    private let material = NSVisualEffectView()
    private let tabStrip = NSView()
    private let tabTitle = NSTextField(labelWithString: "Terminal")
    private let left = NSView()
    private let inspector = NSStackView()
    private let attention = NSStackView()
    private let blocks = NSStackView()
    private let composer: ComposerBridgeView
    private let workspacesButton = NSButton(title: "Workspaces", target: nil, action: nil)
    private let tabsButton = NSButton(title: "Tabs", target: nil, action: nil)
    private let recoveryLabel = NSTextField(labelWithString: "")
    private let leftItems = NSStackView()
    private let inspectorColumn = NSView()
    private let centerColumn = NSView()
    private var recoveryTimer: Timer?
    private var nativeBlocks: [NativeBlockRecord] = []
    private var lastSnapshotGeneration: UInt64 = .max
    private var lastEligibility: UInt16 = .max

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

        configureChromeButtons()
        tabStrip.translatesAutoresizingMaskIntoConstraints = false
        tabStrip.wantsLayer = true
        tabStrip.setAccessibilityElement(true)
        tabStrip.setAccessibilityRole(.group)
        tabStrip.setAccessibilityIdentifier("seyal-tab-strip")
        tabTitle.font = .systemFont(ofSize: 13, weight: .semibold)
        tabTitle.translatesAutoresizingMaskIntoConstraints = false
        tabStrip.addSubview(tabTitle)

        left.translatesAutoresizingMaskIntoConstraints = false
        left.wantsLayer = true
        leftItems.orientation = .vertical
        leftItems.alignment = .leading
        leftItems.spacing = 2
        leftItems.translatesAutoresizingMaskIntoConstraints = false
        left.addSubview(workspacesButton)
        left.addSubview(tabsButton)
        left.addSubview(leftItems)
        workspacesButton.translatesAutoresizingMaskIntoConstraints = false
        tabsButton.translatesAutoresizingMaskIntoConstraints = false

        inspector.orientation = .vertical
        inspector.alignment = .leading
        inspector.spacing = 6
        inspector.translatesAutoresizingMaskIntoConstraints = false
        expose(inspector, identifier: "seyal-inspector")
        attention.orientation = .vertical
        attention.alignment = .leading
        expose(attention, identifier: "seyal-attention")
        recoveryLabel.font = .systemFont(ofSize: 11, weight: .regular)
        recoveryLabel.tag = 2
        recoveryLabel.setAccessibilityElement(true)
        recoveryLabel.setAccessibilityIdentifier("seyal-recovery")
        recoveryLabel.stringValue = "disconnected"
        recoveryLabel.translatesAutoresizingMaskIntoConstraints = false

        inspectorColumn.translatesAutoresizingMaskIntoConstraints = false
        inspectorColumn.wantsLayer = true
        inspectorColumn.addSubview(inspector)
        inspectorColumn.addSubview(attention)
        inspectorColumn.addSubview(recoveryLabel)

        blocks.orientation = .vertical
        blocks.alignment = .leading
        blocks.spacing = 8
        blocks.translatesAutoresizingMaskIntoConstraints = false
        expose(blocks, identifier: "seyal-blocks")
        composer.setAccessibilityIdentifier("seyal-composer")

        centerColumn.translatesAutoresizingMaskIntoConstraints = false
        centerColumn.wantsLayer = true
        centerColumn.addSubview(blocks)
        centerColumn.addSubview(pane)
        centerColumn.addSubview(composer)

        addSubview(tabStrip)
        addSubview(left)
        addSubview(centerColumn)
        addSubview(inspectorColumn)

        pane.setContentHuggingPriority(.defaultLow, for: .vertical)
        pane.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
        composer.setContentHuggingPriority(.required, for: .vertical)
        blocks.setContentHuggingPriority(.defaultHigh, for: .vertical)
        let paneFill = pane.heightAnchor.constraint(greaterThanOrEqualToConstant: 240)

        NSLayoutConstraint.activate([
            material.leadingAnchor.constraint(equalTo: leadingAnchor),
            material.trailingAnchor.constraint(equalTo: trailingAnchor),
            material.topAnchor.constraint(equalTo: topAnchor),
            material.bottomAnchor.constraint(equalTo: bottomAnchor),

            tabStrip.leadingAnchor.constraint(equalTo: leadingAnchor),
            tabStrip.trailingAnchor.constraint(equalTo: trailingAnchor),
            tabStrip.topAnchor.constraint(equalTo: topAnchor),
            tabStrip.heightAnchor.constraint(equalToConstant: 48),
            tabTitle.leadingAnchor.constraint(equalTo: tabStrip.leadingAnchor, constant: 236),
            tabTitle.centerYAnchor.constraint(equalTo: tabStrip.centerYAnchor),

            left.leadingAnchor.constraint(equalTo: leadingAnchor),
            left.topAnchor.constraint(equalTo: tabStrip.bottomAnchor),
            left.bottomAnchor.constraint(equalTo: bottomAnchor),
            left.widthAnchor.constraint(equalToConstant: 220),
            workspacesButton.leadingAnchor.constraint(equalTo: left.leadingAnchor, constant: 10),
            workspacesButton.topAnchor.constraint(equalTo: left.topAnchor, constant: 10),
            tabsButton.leadingAnchor.constraint(equalTo: workspacesButton.trailingAnchor, constant: 8),
            tabsButton.centerYAnchor.constraint(equalTo: workspacesButton.centerYAnchor),
            leftItems.leadingAnchor.constraint(equalTo: left.leadingAnchor, constant: 10),
            leftItems.trailingAnchor.constraint(equalTo: left.trailingAnchor, constant: -10),
            leftItems.topAnchor.constraint(equalTo: workspacesButton.bottomAnchor, constant: 12),
            leftItems.bottomAnchor.constraint(lessThanOrEqualTo: left.bottomAnchor, constant: -10),

            inspectorColumn.trailingAnchor.constraint(equalTo: trailingAnchor),
            inspectorColumn.topAnchor.constraint(equalTo: tabStrip.bottomAnchor),
            inspectorColumn.bottomAnchor.constraint(equalTo: bottomAnchor),
            inspectorColumn.widthAnchor.constraint(equalToConstant: 248),
            inspector.leadingAnchor.constraint(equalTo: inspectorColumn.leadingAnchor, constant: 10),
            inspector.trailingAnchor.constraint(equalTo: inspectorColumn.trailingAnchor, constant: -10),
            inspector.topAnchor.constraint(equalTo: inspectorColumn.topAnchor, constant: 10),
            attention.leadingAnchor.constraint(equalTo: inspector.leadingAnchor),
            attention.trailingAnchor.constraint(equalTo: inspector.trailingAnchor),
            attention.topAnchor.constraint(equalTo: inspector.bottomAnchor, constant: 12),
            recoveryLabel.leadingAnchor.constraint(equalTo: inspector.leadingAnchor),
            recoveryLabel.trailingAnchor.constraint(equalTo: inspector.trailingAnchor),
            recoveryLabel.bottomAnchor.constraint(equalTo: inspectorColumn.bottomAnchor, constant: -10),

            centerColumn.leadingAnchor.constraint(equalTo: left.trailingAnchor),
            centerColumn.trailingAnchor.constraint(equalTo: inspectorColumn.leadingAnchor),
            centerColumn.topAnchor.constraint(equalTo: tabStrip.bottomAnchor),
            centerColumn.bottomAnchor.constraint(equalTo: bottomAnchor),
            blocks.leadingAnchor.constraint(equalTo: centerColumn.leadingAnchor, constant: 12),
            blocks.trailingAnchor.constraint(equalTo: centerColumn.trailingAnchor, constant: -12),
            blocks.topAnchor.constraint(equalTo: centerColumn.topAnchor, constant: 10),
            blocks.heightAnchor.constraint(greaterThanOrEqualToConstant: 8),
            pane.leadingAnchor.constraint(equalTo: centerColumn.leadingAnchor, constant: 8),
            pane.trailingAnchor.constraint(equalTo: centerColumn.trailingAnchor, constant: -8),
            pane.topAnchor.constraint(equalTo: blocks.bottomAnchor, constant: 8),
            paneFill,
            composer.leadingAnchor.constraint(equalTo: centerColumn.leadingAnchor, constant: 12),
            composer.trailingAnchor.constraint(equalTo: centerColumn.trailingAnchor, constant: -12),
            composer.topAnchor.constraint(equalTo: pane.bottomAnchor, constant: 8),
            composer.bottomAnchor.constraint(equalTo: centerColumn.bottomAnchor, constant: -10),
        ])

        composer.onSubmitComposer = { [weak self] command in
            self?.pane.inputSurface.terminalSubmitComposerCommand(command) ?? -10
        }
        composer.onSubmitRaw = { [weak self] command in
            self?.pane.inputSurface.terminalSubmitCommittedText(command) ?? -10
        }
        pane.inputSurface.onRequestComposerFocus = { [weak self] in
            self?.composer.focusEditor()
        }
        pane.inputSurface.onTimelineChanged = { [weak self] records in
            self?.nativeBlocks = records
            self?.rebuildBlocks()
        }
        pane.inputSurface.onComposerResultChanged = { [weak self] result in
            let accepted = result.code == .accepted || result.code == .unsupported
            self?.composer.applyComposerResult(requestID: result.requestID, accepted: accepted)
            self?.reconcileChrome()
        }
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
        routeFocus()
    }

    func requestQuit() { pane.requestQuit() }
    func detachForTermination() { pane.detachForTermination() }

    func reconcileChrome() {
        let snapshot = seyal_app_snapshot(pane.appHandle)
        let eligibilityChanged = snapshot.eligibility != lastEligibility
        if snapshot.generation == lastSnapshotGeneration && !eligibilityChanged {
        composer.reconcile()
        driveRecovery()
        rebuildBlocksIfNeeded()
        return
        }
        lastSnapshotGeneration = snapshot.generation
        let chrome = seyal_app_chrome(pane.appHandle)
        let shell = seyal_app_shell(pane.appHandle)
        workspacesButton.state = chrome.left_panel == 0 ? .on : .off
        tabsButton.state = chrome.left_panel == 1 ? .on : .off
        rebuildLeft(shell: shell, leftPanel: chrome.left_panel)
        rebuildInspector(chrome)
        rebuildTabStrip(shell: shell)
        rebuildBlocks()
        recoveryLabel.stringValue = recoveryText(snapshot)
        composer.reconcile()
        driveRecovery()
        if eligibilityChanged {
            lastEligibility = snapshot.eligibility
            routeFocus()
        }
        applyTheme()
    }

    func routeFocus() {
        let snapshot = seyal_app_snapshot(pane.appHandle)
        let composerSnap = seyal_app_composer(pane.appHandle)
        if snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_FLOW.rawValue),
           composerSnap.mode != UInt16(SEYAL_APP_COMPOSER_HIDDEN.rawValue)
        {
            composer.focusEditor()
        } else if snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_RAW.rawValue)
            || snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_TUI.rawValue)
        {
            window?.makeFirstResponder(pane.inputSurface)
        }
    }

    private func rebuildTabStrip(shell: SeyalAppShell) {
        if shell.tab_count > 0 {
            let row = seyal_app_shell_row(pane.appHandle, UInt16(SEYAL_APP_ROW_TAB), 0)
            tabTitle.stringValue = copyUTF8(row.title, row.title_len) ?? "Terminal"
        }
    }

    private func rebuildLeft(shell: SeyalAppShell, leftPanel: UInt16) {
        leftItems.arrangedSubviews.forEach { $0.removeFromSuperview() }
        if leftPanel == 0 {
            for index in 0..<Int(shell.workspace_count) {
                let row = seyal_app_shell_row(pane.appHandle, UInt16(SEYAL_APP_ROW_WORKSPACE), UInt32(index))
                leftItems.addArrangedSubview(
                    rowButton(
                        title: copyUTF8(row.title, row.title_len) ?? "Workspace",
                        detail: copyUTF8(row.detail, row.detail_len),
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
                        detail: copyUTF8(row.detail, row.detail_len),
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
                    detail: nil,
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
            let caption = NSTextField(labelWithString: title)
            caption.font = .systemFont(ofSize: 10, weight: .medium)
            caption.tag = 2
            let body = NSTextField(labelWithString: value)
            body.font = .systemFont(ofSize: 12, weight: .regular)
            body.maximumNumberOfLines = 2
            inspector.addArrangedSubview(caption)
            inspector.addArrangedSubview(body)
        }
        for index in 0..<Int(chrome.attention_count) {
            let row = seyal_app_chrome_row(pane.appHandle, UInt16(SEYAL_APP_ROW_ATTENTION), UInt32(index))
            let identity = copyUTF8(row.title, row.title_len) ?? ""
            let title = copyUTF8(row.detail, row.detail_len) ?? identity
            let button = borderlessButton(title: title, action: #selector(openAttention(_:)))
            button.setAccessibilityIdentifier("seyal-attention-\(index)")
            button.identifier = NSUserInterfaceItemIdentifier(identity)
            attention.addArrangedSubview(button)
        }
        for index in 0..<Int(chrome.agent_count) {
            let row = seyal_app_chrome_row(pane.appHandle, UInt16(SEYAL_APP_ROW_AGENT), UInt32(index))
            let identity = copyUTF8(row.title, row.title_len) ?? ""
            let name = copyUTF8(row.detail, row.detail_len) ?? identity
            let button = borderlessButton(title: name, action: #selector(selectAgent(_:)))
            button.setAccessibilityIdentifier("seyal-agent-\(index)")
            button.identifier = NSUserInterfaceItemIdentifier(identity)
            button.state = row.flags & UInt16(SEYAL_APP_ROW_SELECTED) != 0 ? .on : .off
            attention.addArrangedSubview(button)
        }
    }

    private func rebuildBlocksIfNeeded() {
        let records = pane.inputSurface.currentTimeline()
        if records.map(\.id) != nativeBlocks.map(\.id)
            || records.map(\.state) != nativeBlocks.map(\.state)
        {
            nativeBlocks = records
            rebuildBlocks()
        }
    }

    private func rebuildBlocks() {
        blocks.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let records = nativeBlocks.isEmpty ? pane.inputSurface.currentTimeline() : nativeBlocks
        nativeBlocks = records
        blocks.isHidden = false
        for (index, record) in records.enumerated() {
            let card = CommandBlockView(record: record)
            card.setAccessibilityIdentifier("seyal-block-\(index)")
            blocks.addArrangedSubview(card)
        }
    }

    private func applyTheme() {
        let theme = NativeThemeRealization.theme(for: effectiveAppearance)
        NativeThemeRealization.apply(
            to: self,
            material: material,
            appearance: effectiveAppearance
        )
        left.layer?.backgroundColor = theme.utility.cgColor
        inspectorColumn.layer?.backgroundColor = theme.utility.cgColor
        tabStrip.layer?.backgroundColor = theme.container.cgColor
        centerColumn.layer?.backgroundColor = theme.canvas.cgColor
        left.layer?.borderWidth = 0
        composer.apply(theme: theme)
        for view in blocks.arrangedSubviews {
            (view as? CommandBlockView)?.apply(theme: theme)
        }
    }

    private func recoveryText(_ snapshot: SeyalAppSnapshot) -> String {
        let stage: String
        switch snapshot.recovery_stage {
        case 6: stage = "connected"
        case 7: stage = "recovery exhausted"
        case 8: stage = "blocked"
        case 4, 5: stage = "restoring"
        case 0: stage = "disconnected"
        default: stage = "connecting"
        }
        if snapshot.recovery_stage == 6 {
            return stage
        }
        return "\(stage) · attempts \(snapshot.recovery_attempts)"
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
            // CompleteRecovery replaces the effect queue. Acking here would
            // drop LaunchHelper without spawning Runtime.
            completeRecovery(connected: pane.inputSurface.terminalBridgeIsConnected)
            driveRecovery()
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
            ackRecovery()
        case UInt32(SEYAL_APP_RECOVERY_EFFECT_LAUNCH_HELPER.rawValue):
            _ = BundledRuntimeLauncher().launch()
            ackRecovery()
            driveRecovery()
        case UInt32(SEYAL_APP_RECOVERY_EFFECT_DISPOSE_HANDLE.rawValue):
            seyal_bridge_disconnect_handle(seyal_app_recovery_param(pane.appHandle))
            ackRecovery()
            driveRecovery()
        default:
            break
        }
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

    private func configureChromeButtons() {
        styleSwitcher(workspacesButton, identifier: "seyal-left-workspaces", action: #selector(showWorkspaces))
        styleSwitcher(tabsButton, identifier: "seyal-left-tabs", action: #selector(showTabs))
    }

    private func styleSwitcher(_ button: NSButton, identifier: String, action: Selector) {
        button.setButtonType(.toggle)
        button.bezelStyle = .inline
        button.isBordered = false
        button.font = .systemFont(ofSize: 11, weight: .semibold)
        button.setAccessibilityIdentifier(identifier)
        button.target = self
        button.action = action
    }

    private func borderlessButton(title: String, action: Selector) -> NSButton {
        let button = NSButton(title: title, target: self, action: action)
        button.bezelStyle = .inline
        button.isBordered = false
        button.font = .systemFont(ofSize: 12, weight: .regular)
        button.alignment = .left
        return button
    }

    private func rowButton(
        title: String,
        detail: String?,
        identifier: String,
        selected: Bool,
        action: Selector,
        kind: UInt16,
        idLo: UInt64,
        idHi: UInt64
    ) -> IdentityButton {
        let label = detail.flatMap { $0.isEmpty ? nil : $0 }.map { "\(title)  \($0)" } ?? title
        let button = IdentityButton(title: label, target: self, action: action)
        button.idLo = idLo
        button.idHi = idHi
        button.kind = kind
        button.bezelStyle = .inline
        button.isBordered = false
        button.font = .systemFont(ofSize: 12, weight: selected ? .semibold : .regular)
        button.alignment = .left
        button.setAccessibilityIdentifier(identifier)
        button.state = selected ? .on : .off
        return button
    }

    private func expose(_ view: NSView, identifier: String) {
        view.setAccessibilityElement(true)
        view.setAccessibilityRole(.group)
        view.setAccessibilityIdentifier(identifier)
    }
}

private final class IdentityButton: NSButton {
    var kind: UInt16 = 0
    var idLo: UInt64 = 0
    var idHi: UInt64 = 0
}

private final class CommandBlockView: NSView {
    private let command = NSTextField(labelWithString: "")
    private let status = NSTextField(labelWithString: "")
    private let seam = NSView()

    init(record: NativeBlockRecord) {
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true
        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        command.stringValue = record.command.isEmpty ? "command" : record.command
        setAccessibilityLabel(command.stringValue)
        command.font = .monospacedSystemFont(ofSize: 12, weight: .medium)
        command.translatesAutoresizingMaskIntoConstraints = false
        status.stringValue = record.state == .running ? "running" : (record.exitStatus == 0 ? "ok" : "exit \(record.exitStatus)")
        status.font = .systemFont(ofSize: 11, weight: .regular)
        status.tag = 1
        status.translatesAutoresizingMaskIntoConstraints = false
        seam.translatesAutoresizingMaskIntoConstraints = false
        seam.wantsLayer = true
        addSubview(seam)
        addSubview(command)
        addSubview(status)
        NSLayoutConstraint.activate([
            heightAnchor.constraint(greaterThanOrEqualToConstant: 36),
            seam.leadingAnchor.constraint(equalTo: leadingAnchor),
            seam.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            seam.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
            seam.widthAnchor.constraint(equalToConstant: 2),
            command.leadingAnchor.constraint(equalTo: seam.trailingAnchor, constant: 10),
            command.centerYAnchor.constraint(equalTo: centerYAnchor),
            status.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            status.centerYAnchor.constraint(equalTo: centerYAnchor),
            command.trailingAnchor.constraint(lessThanOrEqualTo: status.leadingAnchor, constant: -8),
        ])
        seam.identifier = NSUserInterfaceItemIdentifier(record.state == .running ? "running" : "completed")
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("CommandBlockView is programmatic")
    }

    func apply(theme: NativeTheme) {
        layer?.backgroundColor = theme.container.cgColor
        command.textColor = theme.text
        status.textColor = theme.secondary
        let running = seam.identifier?.rawValue == "running"
        seam.layer?.backgroundColor = running ? theme.warning.cgColor : theme.success.cgColor
    }
}

private func copyUTF8(_ pointer: UnsafePointer<UInt8>?, _ length: UInt32) -> String? {
    guard length > 0, let pointer else { return nil }
    return String(decoding: UnsafeBufferPointer(start: pointer, count: Int(length)), as: UTF8.self)
}
