import AppKit
import QuartzCore

/// Thin AppKit projection of Rust shell/chrome/composer/recovery. No writable product model.
@MainActor
final class ProductChromeHostView: NSView {
    let pane: ThinPaneHostView
    private let material = NSVisualEffectView()
    private let tabStrip = NSView()
    private let tabItems = NSStackView()
    private let chromeActions = NSStackView()
    private let left = NSView()
    private let inspectorModes = NSStackView()
    private let inspector = NSStackView()
    private let attention = NSStackView()
    private let transcript = NSScrollView()
    private let blocks = NSStackView()
    private let composer: ComposerBridgeView
    private let workspacesButton = NSButton(title: "Workspaces", target: nil, action: nil)
    private let tabsButton = NSButton(title: "Tabs", target: nil, action: nil)
    private let newTabButton = NSButton(title: "+", target: nil, action: nil)
    private let splitRightButton = NSButton(title: "Split Right", target: nil, action: nil)
    private let splitDownButton = NSButton(title: "Split Down", target: nil, action: nil)
    private let closePaneButton = NSButton(title: "Close Pane", target: nil, action: nil)
    private let attentionBell = NSButton(title: "Attention", target: nil, action: nil)
    private let attentionPopover = NSStackView()
    private let paletteOverlay = NSView()
    private let paletteField = NSTextField(string: "")
    private let paletteList = NSStackView()
    private let recoveryLabel = NSTextField(labelWithString: "")
    private let leftItems = NSStackView()
    private let inspectorColumn = NSView()
    private let centerColumn = NSView()
    private let agentsCenter = NSStackView()
    private let multipaneBoard = MultipaneBoardView()
    private let liveSurfaceHost = NSView()
    private var recoveryTimer: Timer?
    private var lastSnapshotGeneration: UInt64 = .max
    private var lastEligibility: UInt16 = .max
    private var lastProjectedExecution = (lo: UInt64(0), hi: UInt64(0))
    private var lastBlockCount: Int = 0
    private var isReconcilingChrome = false
    private var isApplyingPaletteQuery = false
    private var blockCards: [UInt64: CommandBlockView] = [:]
    private var transcriptFrameRevision: UInt64 = 0
    private var paneFollowsTranscript: [NSLayoutConstraint] = []
    private var paneFillsCenter: [NSLayoutConstraint] = []
    private var centerLeadingHost: NSLayoutConstraint!
    private var centerTrailingHost: NSLayoutConstraint!
    private var centerTopHost: NSLayoutConstraint!
    private var centerLeadingLeft: NSLayoutConstraint!
    private var centerTrailingInspector: NSLayoutConstraint!
    private var centerTopTab: NSLayoutConstraint!
    private var lastChromeDepths: [UInt16: UInt16] = [:]
    private let coreCenterButton = NSButton(title: "Core", target: nil, action: nil)
    private let agentsCenterButton = NSButton(title: "Agents", target: nil, action: nil)

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
        tabItems.orientation = .horizontal
        tabItems.alignment = .centerY
        tabItems.spacing = 6
        tabItems.translatesAutoresizingMaskIntoConstraints = false
        tabItems.setAccessibilityIdentifier("seyal-tab-items")
        chromeActions.orientation = .horizontal
        chromeActions.alignment = .centerY
        chromeActions.spacing = 8
        chromeActions.translatesAutoresizingMaskIntoConstraints = false
        chromeActions.setAccessibilityIdentifier("seyal-chrome-actions")
        tabStrip.addSubview(tabItems)
        tabStrip.addSubview(chromeActions)
        inspectorModes.orientation = .horizontal
        inspectorModes.alignment = .centerY
        inspectorModes.spacing = 4
        inspectorModes.translatesAutoresizingMaskIntoConstraints = false
        inspectorModes.setAccessibilityIdentifier("seyal-inspector-modes")

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
        expose(attention, identifier: "seyal-agents")
        attention.isHidden = true
        attentionPopover.orientation = .vertical
        attentionPopover.alignment = .leading
        attentionPopover.spacing = 4
        attentionPopover.translatesAutoresizingMaskIntoConstraints = false
        attentionPopover.wantsLayer = true
        attentionPopover.layer?.backgroundColor = NSColor.windowBackgroundColor.withAlphaComponent(0.96).cgColor
        attentionPopover.layer?.cornerRadius = 8
        attentionPopover.layer?.borderWidth = 1
        attentionPopover.layer?.borderColor = NSColor.separatorColor.cgColor
        attentionPopover.isHidden = true
        expose(attentionPopover, identifier: "seyal-attention-popover")
        addSubview(attentionPopover)

        paletteOverlay.translatesAutoresizingMaskIntoConstraints = false
        paletteOverlay.wantsLayer = true
        paletteOverlay.layer?.backgroundColor = NSColor.windowBackgroundColor.withAlphaComponent(0.97).cgColor
        paletteOverlay.layer?.cornerRadius = 10
        paletteOverlay.layer?.borderWidth = 1
        paletteOverlay.layer?.borderColor = NSColor.separatorColor.cgColor
        paletteOverlay.isHidden = true
        expose(paletteOverlay, identifier: "seyal-command-palette")
        paletteField.placeholderString = "Type a command"
        paletteField.font = .systemFont(ofSize: 14, weight: .regular)
        paletteField.isBordered = true
        paletteField.isBezeled = true
        paletteField.bezelStyle = .roundedBezel
        paletteField.focusRingType = .exterior
        paletteField.translatesAutoresizingMaskIntoConstraints = false
        paletteField.setAccessibilityIdentifier("seyal-command-palette-query")
        paletteField.delegate = self
        paletteList.orientation = .vertical
        paletteList.alignment = .leading
        paletteList.spacing = 2
        paletteList.translatesAutoresizingMaskIntoConstraints = false
        paletteList.setAccessibilityIdentifier("seyal-command-palette-list")
        paletteOverlay.addSubview(paletteField)
        paletteOverlay.addSubview(paletteList)
        addSubview(paletteOverlay)

        recoveryLabel.font = .systemFont(ofSize: 11, weight: .regular)
        recoveryLabel.tag = 2
        recoveryLabel.setAccessibilityElement(true)
        recoveryLabel.setAccessibilityIdentifier("seyal-recovery")
        recoveryLabel.stringValue = "disconnected"
        recoveryLabel.translatesAutoresizingMaskIntoConstraints = false

        inspectorColumn.translatesAutoresizingMaskIntoConstraints = false
        inspectorColumn.wantsLayer = true
        inspectorColumn.addSubview(inspectorModes)
        inspectorColumn.addSubview(inspector)
        inspectorColumn.addSubview(attention)

        blocks.orientation = .vertical
        blocks.alignment = .width
        blocks.spacing = 22
        blocks.edgeInsets = NSEdgeInsets(top: 8, left: 0, bottom: 8, right: 0)
        blocks.translatesAutoresizingMaskIntoConstraints = false
        expose(blocks, identifier: "seyal-blocks")
        composer.setAccessibilityIdentifier("seyal-composer")

        let clip = TranscriptClipView()
        clip.drawsBackground = false
        clip.copiesOnScroll = false
        clip.postsBoundsChangedNotifications = true
        transcript.contentView = clip
        transcript.drawsBackground = false
        transcript.borderType = .noBorder
        transcript.hasVerticalScroller = true
        transcript.hasHorizontalScroller = false
        transcript.autohidesScrollers = true
        transcript.automaticallyAdjustsContentInsets = false
        transcript.translatesAutoresizingMaskIntoConstraints = false
        transcript.documentView = blocks
        transcript.setAccessibilityRole(.scrollArea)
        transcript.setAccessibilityIdentifier("seyal-blocks-scroll")

        centerColumn.translatesAutoresizingMaskIntoConstraints = false
        centerColumn.wantsLayer = true
        // Transcript chrome sits under the Pane Metal compositor. Flow clears
        // the drawable to transparent and paints only Block-body clips, so
        // command headers remain AppKit while output glyphs composite on top.
        multipaneBoard.chromeHost = self
        multipaneBoard.translatesAutoresizingMaskIntoConstraints = false
        centerColumn.addSubview(multipaneBoard)
        multipaneBoard.installLiveSubviews([transcript, pane, composer])

        agentsCenter.orientation = .vertical
        agentsCenter.alignment = .leading
        agentsCenter.spacing = 4
        agentsCenter.translatesAutoresizingMaskIntoConstraints = false
        agentsCenter.edgeInsets = NSEdgeInsets(top: 12, left: 16, bottom: 12, right: 16)
        agentsCenter.isHidden = true
        expose(agentsCenter, identifier: "seyal-agents-center")
        centerColumn.addSubview(agentsCenter)

        addSubview(tabStrip)
        addSubview(left)
        addSubview(centerColumn)
        addSubview(inspectorColumn)
        addSubview(recoveryLabel)
        recoveryLabel.alphaValue = 0
        recoveryLabel.setAccessibilityElement(true)

        pane.setContentHuggingPriority(.defaultLow, for: .vertical)
        pane.setContentCompressionResistancePriority(.defaultLow, for: .vertical)
        composer.setContentHuggingPriority(.required, for: .vertical)
        transcript.setContentHuggingPriority(NSLayoutConstraint.Priority(1), for: .vertical)
        transcript.setContentCompressionResistancePriority(.defaultLow, for: .vertical)

        NSLayoutConstraint.activate([
            material.leadingAnchor.constraint(equalTo: leadingAnchor),
            material.trailingAnchor.constraint(equalTo: trailingAnchor),
            material.topAnchor.constraint(equalTo: topAnchor),
            material.bottomAnchor.constraint(equalTo: bottomAnchor),

            tabStrip.leadingAnchor.constraint(equalTo: leadingAnchor),
            tabStrip.trailingAnchor.constraint(equalTo: trailingAnchor),
            tabStrip.topAnchor.constraint(equalTo: topAnchor),
            tabStrip.heightAnchor.constraint(equalToConstant: 48),
            tabItems.leadingAnchor.constraint(equalTo: tabStrip.leadingAnchor, constant: 236),
            tabItems.centerYAnchor.constraint(equalTo: tabStrip.centerYAnchor),
            tabItems.trailingAnchor.constraint(lessThanOrEqualTo: chromeActions.leadingAnchor, constant: -12),
            chromeActions.trailingAnchor.constraint(equalTo: tabStrip.trailingAnchor, constant: -12),
            chromeActions.centerYAnchor.constraint(equalTo: tabStrip.centerYAnchor),

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
            inspectorModes.leadingAnchor.constraint(equalTo: inspectorColumn.leadingAnchor, constant: 10),
            inspectorModes.trailingAnchor.constraint(equalTo: inspectorColumn.trailingAnchor, constant: -10),
            inspectorModes.topAnchor.constraint(equalTo: inspectorColumn.topAnchor, constant: 10),
            inspector.leadingAnchor.constraint(equalTo: inspectorColumn.leadingAnchor, constant: 10),
            inspector.trailingAnchor.constraint(equalTo: inspectorColumn.trailingAnchor, constant: -10),
            inspector.topAnchor.constraint(equalTo: inspectorModes.bottomAnchor, constant: 8),
            attention.leadingAnchor.constraint(equalTo: inspector.leadingAnchor),
            attention.trailingAnchor.constraint(equalTo: inspector.trailingAnchor),
            attention.topAnchor.constraint(equalTo: inspector.bottomAnchor, constant: 12),
            attentionPopover.topAnchor.constraint(equalTo: tabStrip.bottomAnchor, constant: 4),
            attentionPopover.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            attentionPopover.widthAnchor.constraint(equalToConstant: 280),
            paletteOverlay.centerXAnchor.constraint(equalTo: centerXAnchor),
            paletteOverlay.topAnchor.constraint(equalTo: tabStrip.bottomAnchor, constant: 24),
            paletteOverlay.widthAnchor.constraint(equalToConstant: 420),
            paletteField.leadingAnchor.constraint(equalTo: paletteOverlay.leadingAnchor, constant: 12),
            paletteField.trailingAnchor.constraint(equalTo: paletteOverlay.trailingAnchor, constant: -12),
            paletteField.topAnchor.constraint(equalTo: paletteOverlay.topAnchor, constant: 12),
            paletteList.leadingAnchor.constraint(equalTo: paletteField.leadingAnchor),
            paletteList.trailingAnchor.constraint(equalTo: paletteField.trailingAnchor),
            paletteList.topAnchor.constraint(equalTo: paletteField.bottomAnchor, constant: 8),
            paletteList.bottomAnchor.constraint(equalTo: paletteOverlay.bottomAnchor, constant: -12),
            recoveryLabel.leadingAnchor.constraint(equalTo: leadingAnchor),
            recoveryLabel.topAnchor.constraint(equalTo: topAnchor),
            recoveryLabel.widthAnchor.constraint(equalToConstant: 1),
            recoveryLabel.heightAnchor.constraint(equalToConstant: 1),

            centerColumn.bottomAnchor.constraint(equalTo: bottomAnchor),
            multipaneBoard.leadingAnchor.constraint(equalTo: centerColumn.leadingAnchor),
            multipaneBoard.trailingAnchor.constraint(equalTo: centerColumn.trailingAnchor),
            multipaneBoard.topAnchor.constraint(equalTo: centerColumn.topAnchor),
            multipaneBoard.bottomAnchor.constraint(equalTo: centerColumn.bottomAnchor),
            agentsCenter.leadingAnchor.constraint(equalTo: centerColumn.leadingAnchor),
            agentsCenter.trailingAnchor.constraint(equalTo: centerColumn.trailingAnchor),
            agentsCenter.topAnchor.constraint(equalTo: centerColumn.topAnchor),
            agentsCenter.bottomAnchor.constraint(lessThanOrEqualTo: centerColumn.bottomAnchor),
            blocks.topAnchor.constraint(equalTo: transcript.contentView.topAnchor),
            blocks.leadingAnchor.constraint(equalTo: transcript.contentView.leadingAnchor),
            blocks.widthAnchor.constraint(equalTo: transcript.contentView.widthAnchor),
        ])
        centerLeadingHost = centerColumn.leadingAnchor.constraint(equalTo: leadingAnchor)
        centerTrailingHost = centerColumn.trailingAnchor.constraint(equalTo: trailingAnchor)
        centerTopHost = centerColumn.topAnchor.constraint(equalTo: topAnchor)
        centerLeadingLeft = centerColumn.leadingAnchor.constraint(equalTo: left.trailingAnchor)
        centerTrailingInspector = centerColumn.trailingAnchor.constraint(
            equalTo: inspectorColumn.leadingAnchor
        )
        centerTopTab = centerColumn.topAnchor.constraint(equalTo: tabStrip.bottomAnchor)
        paneFollowsTranscript = [
            pane.leadingAnchor.constraint(equalTo: transcript.leadingAnchor),
            pane.trailingAnchor.constraint(equalTo: transcript.trailingAnchor),
            pane.topAnchor.constraint(equalTo: transcript.topAnchor),
            pane.bottomAnchor.constraint(equalTo: transcript.bottomAnchor),
        ]
        paneFillsCenter = [
            pane.leadingAnchor.constraint(equalTo: multipaneBoard.liveContentView.leadingAnchor),
            pane.trailingAnchor.constraint(equalTo: multipaneBoard.liveContentView.trailingAnchor),
            pane.topAnchor.constraint(equalTo: multipaneBoard.liveContentView.topAnchor),
            pane.bottomAnchor.constraint(equalTo: multipaneBoard.liveContentView.bottomAnchor),
        ]
        NSLayoutConstraint.activate(paneFollowsTranscript)
        applyShellChrome(seyal_app_chrome(pane.appHandle))

        composer.onSubmitComposer = { [weak self] command in
            self?.pane.inputSurface.terminalSubmitComposerCommand(command) ?? -10
        }
        composer.onSubmitRaw = { [weak self] command in
            self?.pane.inputSurface.terminalSubmitCommittedText(command) ?? -10
        }
        pane.inputSurface.onRequestComposerFocus = { [weak self] in
            self?.composer.focusEditor()
        }
        pane.inputSurface.onTimelineChanged = { [weak self] in
            self?.projectRuntimeBlocks()
            self?.reconcileChrome()
            self?.refreshRunningBlockOutput()
        }
        pane.inputSurface.onHistoryRangeChanged = { [weak self] range in
            self?.applyHistoryRange(range)
        }
        pane.inputSurface.onComposerResultChanged = { [weak self] result in
            let accepted = result.code == .accepted || result.code == .unsupported
            self?.composer.applyComposerResult(requestID: result.requestID, accepted: accepted)
            self?.reconcileChrome()
        }
        pane.onProductChanged = { [weak self] in
            self?.reconcileChrome()
        }
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(transcriptDidScroll),
            name: NSView.boundsDidChangeNotification,
            object: clip
        )
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("ProductChromeHostView is programmatic")
    }

    @objc private func transcriptDidScroll() {
        publishBlockOutputFrame()
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
        guard !isReconcilingChrome else { return }
        isReconcilingChrome = true
        defer { isReconcilingChrome = false }
        var snapshot = seyal_app_snapshot(pane.appHandle)
        let bound = (lo: snapshot.execution_lo, hi: snapshot.execution_hi)
        if snapshot.flags & UInt16(SEYAL_APP_SNAP_HAS_EXECUTION) != 0,
           bound != lastProjectedExecution
        {
            lastProjectedExecution = bound
            projectRuntimeBlocks()
            snapshot = seyal_app_snapshot(pane.appHandle)
        }
        let eligibilityChanged = snapshot.eligibility != lastEligibility
        if snapshot.generation == lastSnapshotGeneration && !eligibilityChanged {
            composer.reconcile()
            applyTheme()
            driveRecovery()
            return
        }
        lastSnapshotGeneration = snapshot.generation
        // Stamp eligibility before Block history requests. Those calls notify
        // bridge status, which used to re-enter here with eligibilityChanged
        // still true and overflow the main-thread stack (nvim TUI takeover).
        if eligibilityChanged {
            lastEligibility = snapshot.eligibility
        }
        let chrome = seyal_app_chrome(pane.appHandle)
        applyShellChrome(chrome)
        let shell = seyal_app_shell(pane.appHandle)
        workspacesButton.state = chrome.left_panel == 0 ? .on : .off
        tabsButton.state = chrome.left_panel == 1 ? .on : .off
        coreCenterButton.state = chrome.center_surface == 0 ? .on : .off
        agentsCenterButton.state = chrome.center_surface == 1 ? .on : .off
        rebuildLeft(shell: shell, chrome: chrome)
        rebuildInspector(chrome)
        rebuildAgentsCenter(chrome)
        applyCenterSurface(chrome)
        rebuildTabStrip(shell: shell)
        rebuildCommandPalette(chrome)
        multipaneBoard.rebuild(appHandle: pane.appHandle)
        let direct = snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_RAW.rawValue)
            || snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_TUI.rawValue)
        if !direct {
            rebuildBlocks()
        }
        applyTranscriptPresentation(snapshot)
        recoveryLabel.stringValue = recoveryText(snapshot)
        composer.reconcile()
        driveRecovery()
        if eligibilityChanged {
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
        tabItems.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for index in 0..<Int(shell.tab_count) {
            let row = seyal_app_shell_row(pane.appHandle, UInt16(SEYAL_APP_ROW_TAB), UInt32(index))
            let selected = row.flags & UInt16(SEYAL_APP_ROW_SELECTED) != 0
            let button = rowButton(
                title: copyUTF8(row.title, row.title_len) ?? "Tab",
                detail: nil,
                identifier: "seyal-tab-strip-\(index)",
                selected: selected,
                action: #selector(selectTab(_:)),
                kind: UInt16(SEYAL_APP_ACTION_SELECT_TAB.rawValue),
                idLo: row.id_lo,
                idHi: row.id_hi
            )
            button.font = .systemFont(ofSize: 12, weight: selected ? .semibold : .regular)
            tabItems.addArrangedSubview(button)
        }
        tabItems.addArrangedSubview(newTabButton)
    }

    private func rebuildLeft(shell: SeyalAppShell, chrome: SeyalAppChrome) {
        leftItems.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let leftPanel = chrome.left_panel
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
            if chrome.agent_count > 0 {
                let header = NSTextField(labelWithString: "Agents")
                header.font = .systemFont(ofSize: 10, weight: .semibold)
                header.setAccessibilityIdentifier("seyal-left-agents-header")
                leftItems.addArrangedSubview(header)
            }
            for index in 0..<Int(chrome.agent_count) {
                let row = seyal_app_chrome_row(pane.appHandle, UInt16(SEYAL_APP_ROW_AGENT), UInt32(index))
                let identity = copyUTF8(row.title, row.title_len) ?? ""
                let name = copyUTF8(row.detail, row.detail_len) ?? identity
                let button = borderlessButton(title: name, action: #selector(selectAgent(_:)))
                button.setAccessibilityIdentifier("seyal-agent-\(index)")
                button.identifier = NSUserInterfaceItemIdentifier(identity)
                button.state = row.flags & UInt16(SEYAL_APP_ROW_SELECTED) != 0 ? .on : .off
                leftItems.addArrangedSubview(button)
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

    private func rebuildAgentsCenter(_ chrome: SeyalAppChrome) {
        agentsCenter.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let header = NSTextField(labelWithString: "Agents")
        header.font = .systemFont(ofSize: 13, weight: .semibold)
        header.setAccessibilityIdentifier("seyal-agents-center-header")
        agentsCenter.addArrangedSubview(header)
        if chrome.agent_count == 0 {
            let empty = NSTextField(labelWithString: "No agents in this Workspace")
            empty.font = .systemFont(ofSize: 12, weight: .regular)
            empty.textColor = .secondaryLabelColor
            empty.setAccessibilityIdentifier("seyal-agents-center-empty")
            agentsCenter.addArrangedSubview(empty)
            return
        }
        for index in 0..<Int(chrome.agent_count) {
            let row = seyal_app_chrome_row(pane.appHandle, UInt16(SEYAL_APP_ROW_AGENT), UInt32(index))
            let identity = copyUTF8(row.title, row.title_len) ?? ""
            let name = copyUTF8(row.detail, row.detail_len) ?? identity
            let button = borderlessButton(title: name, action: #selector(selectAgent(_:)))
            button.setAccessibilityIdentifier("seyal-agents-center-\(index)")
            button.identifier = NSUserInterfaceItemIdentifier(identity)
            button.state = row.flags & UInt16(SEYAL_APP_ROW_SELECTED) != 0 ? .on : .off
            agentsCenter.addArrangedSubview(button)
        }
    }

    private func applyCenterSurface(_ chrome: SeyalAppChrome) {
        let agents = chrome.center_surface == 1
        agentsCenter.isHidden = !agents
        agentsCenter.setAccessibilityElement(agents)
        multipaneBoard.isHidden = agents
        multipaneBoard.setAccessibilityElement(!agents)
    }

    private func rebuildInspector(_ chrome: SeyalAppChrome) {
        inspectorModes.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for (title, mode, identifier) in [
            ("Context", UInt32(0), "seyal-inspector-mode-context"),
            ("Workspace", UInt32(1), "seyal-inspector-mode-workspace"),
            ("Tab", UInt32(2), "seyal-inspector-mode-tab"),
            ("Pane", UInt32(3), "seyal-inspector-mode-pane"),
            ("Blocks", UInt32(4), "seyal-inspector-mode-blocks"),
        ] {
            let button = borderlessButton(title: title, action: #selector(setInspectorMode(_:)))
            button.setButtonType(.toggle)
            button.tag = Int(mode)
            button.setAccessibilityIdentifier(identifier)
            button.state = chrome.inspector_mode == UInt16(mode) ? .on : .off
            inspectorModes.addArrangedSubview(button)
        }
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
        rebuildAttentionPopover(chrome)
    }

    private func rebuildCommandPalette(_ chrome: SeyalAppChrome) {
        let open = chrome.reserved & UInt32(SEYAL_APP_CHROME_PALETTE_OPEN) != 0
        paletteOverlay.isHidden = !open
        paletteOverlay.setAccessibilityElement(open)
        paletteList.arrangedSubviews.forEach { $0.removeFromSuperview() }
        guard open else { return }
        let queryRow = seyal_app_copy(pane.appHandle, UInt16(SEYAL_APP_COPY_PALETTE_QUERY))
        let query = copyUTF8(queryRow.title, queryRow.title_len) ?? ""
        if !isApplyingPaletteQuery, paletteField.stringValue != query {
            isApplyingPaletteQuery = true
            paletteField.stringValue = query
            isApplyingPaletteQuery = false
        }
        var index = 0
        while true {
            let row = seyal_app_chrome_row(
                pane.appHandle,
                UInt16(SEYAL_APP_ROW_PALETTE),
                UInt32(index)
            )
            if row.title_len == 0 { break }
            let title = copyUTF8(row.detail, row.detail_len)
                ?? copyUTF8(row.title, row.title_len)
                ?? ""
            let selected = row.flags & UInt16(SEYAL_APP_ROW_SELECTED) != 0
            let button = borderlessButton(title: title, action: #selector(runSelectedPaletteCommand))
            button.font = .systemFont(ofSize: 13, weight: selected ? .semibold : .regular)
            button.setAccessibilityIdentifier("seyal-command-palette-\(index)")
            button.state = selected ? .on : .off
            if selected {
                button.wantsLayer = true
                button.layer?.backgroundColor = NSColor.selectedControlColor.withAlphaComponent(0.25).cgColor
            }
            paletteList.addArrangedSubview(button)
            index += 1
        }
        if open, window?.firstResponder !== paletteField {
            window?.makeFirstResponder(paletteField)
        }
    }


    private func rebuildAttentionPopover(_ chrome: SeyalAppChrome) {
        attentionPopover.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let open = chrome.reserved & UInt32(SEYAL_APP_CHROME_ATTENTION_POPOVER_OPEN) != 0
        attentionPopover.isHidden = !open
        attentionPopover.setAccessibilityElement(open)
        let count = Int(chrome.attention_count)
        let badge = count == 0 ? "Attention" : "Attention (\(count))"
        attentionBell.title = badge
        attentionBell.state = open ? .on : .off
        if count == 0 {
            let empty = NSTextField(labelWithString: "No attention items")
            empty.font = .systemFont(ofSize: 12, weight: .regular)
            empty.setAccessibilityIdentifier("seyal-attention-empty")
            attentionPopover.addArrangedSubview(empty)
            return
        }
        for index in 0..<count {
            let row = seyal_app_chrome_row(pane.appHandle, UInt16(SEYAL_APP_ROW_ATTENTION), UInt32(index))
            let identity = copyUTF8(row.title, row.title_len) ?? ""
            let title = copyUTF8(row.detail, row.detail_len) ?? identity
            let button = borderlessButton(title: title, action: #selector(openAttention(_:)))
            button.setAccessibilityIdentifier("seyal-attention-\(index)")
            button.identifier = NSUserInterfaceItemIdentifier(identity)
            attentionPopover.addArrangedSubview(button)
        }
    }

    private func applyShellChrome(_ chrome: SeyalAppChrome) {
        let leftOn = chrome.reserved & UInt32(SEYAL_APP_CHROME_LEFT_VISIBLE) != 0
        let inspectorOn = chrome.reserved & UInt32(SEYAL_APP_CHROME_INSPECTOR_VISIBLE) != 0
        let tabOn = chrome.reserved & UInt32(SEYAL_APP_CHROME_TAB_STRIP_VISIBLE) != 0
        left.isHidden = !leftOn
        left.setAccessibilityElement(leftOn)
        inspectorColumn.isHidden = !inspectorOn
        inspectorColumn.setAccessibilityElement(inspectorOn)
        inspector.setAccessibilityElement(inspectorOn)
        tabStrip.isHidden = !tabOn
        tabStrip.setAccessibilityElement(tabOn)
        centerLeadingHost.isActive = !leftOn
        centerLeadingLeft.isActive = leftOn
        centerTrailingHost.isActive = !inspectorOn
        centerTrailingInspector.isActive = inspectorOn
        centerTopHost.isActive = !tabOn
        centerTopTab.isActive = tabOn
    }

    private func projectRuntimeBlocks() {
        let snapshot = seyal_app_snapshot(pane.appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_APPLY_RUNTIME_BLOCKS.rawValue)
        action.applySnapshotFence(snapshot)
        _ = seyal_app_apply(pane.appHandle, &action)
    }

    private func applyTranscriptPresentation(_ snapshot: SeyalAppSnapshot) {
        let direct = snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_RAW.rawValue)
            || snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_TUI.rawValue)
        transcript.isHidden = direct
        if direct {
            NSLayoutConstraint.deactivate(paneFollowsTranscript)
            NSLayoutConstraint.activate(paneFillsCenter)
            let mode: TerminalPresentationMode =
                snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_TUI.rawValue) ? .tui : .raw
            pane.inputSurface.applyRendererPresentation(.fullPane(mode))
            pane.inputSurface.removeTranscriptRegions(except: [])
            layoutSubtreeIfNeeded()
        } else {
            NSLayoutConstraint.deactivate(paneFillsCenter)
            NSLayoutConstraint.activate(paneFollowsTranscript)
            pane.inputSurface.applyRendererPresentation(.flow())
            layoutSubtreeIfNeeded()
            publishBlockOutputFrame()
        }
    }

    private func rebuildBlocks() {
        blocks.arrangedSubviews.forEach { $0.removeFromSuperview() }
        blockCards.removeAll()
        let composer = seyal_app_composer(pane.appHandle)
        let count = Int(composer.block_count)
        let cellHeight = pane.inputSurface.terminalPresentationCellSize().height
        var retained = Set<UInt64>()
        for index in 0..<count {
            let row = seyal_app_block_row(pane.appHandle, UInt32(index))
            let span = seyal_app_block_span(pane.appHandle, UInt32(index))
            let title = copyUTF8(row.title, row.title_len) ?? "command"
            let detail = copyUTF8(row.detail, row.detail_len) ?? ""
            let promptRow = seyal_app_copy(pane.appHandle, UInt16(SEYAL_APP_COPY_BLOCK_PROMPT))
            let prompt = copyUTF8(promptRow.title, promptRow.title_len) ?? "$"
            let blockID = row.id_lo
            let lines = outputLineCount(span)
            let card = CommandBlockView(
                prompt: prompt,
                title: title,
                detail: detail,
                state: row.flags,
                cellHeight: cellHeight,
                lines: lines,
                idLo: row.id_lo,
                idHi: row.id_hi
            )
            card.setAccessibilityIdentifier("seyal-block-\(index)")
            card.body.setAccessibilityIdentifier("seyal-block-\(index)-body")
            card.onSelect = { [weak self] idLo, idHi in
                self?.selectInspectorBlock(idLo: idLo, idHi: idHi)
            }
            blocks.addArrangedSubview(card)
            if blockID != 0 {
                blockCards[blockID] = card
                retained.insert(blockID)
                requestBlockOutput(blockID: blockID, span: span)
            }
        }
        pane.inputSurface.discardHistoryRequests(except: retained)
        layoutSubtreeIfNeeded()
        publishBlockOutputFrame()
        if count > lastBlockCount {
            scrollTranscriptToLiveEnd()
        }
        lastBlockCount = count
    }

    private func outputLineCount(_ span: SeyalAppBlockSpan) -> Int {
        guard span.start_line > 0 else { return 1 }
        if span.end_line >= span.start_line {
            return Int(min(span.end_line - span.start_line + 1, 512))
        }
        return 8
    }

    private func refreshRunningBlockOutput() {
        let snapshot = seyal_app_snapshot(pane.appHandle)
        if snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_RAW.rawValue)
            || snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_TUI.rawValue)
        {
            return
        }
        let composer = seyal_app_composer(pane.appHandle)
        for index in 0..<Int(composer.block_count) {
            let row = seyal_app_block_row(pane.appHandle, UInt32(index))
            let span = seyal_app_block_span(pane.appHandle, UInt32(index))
            guard row.id_lo != 0, span.start_line > 0, span.end_line == 0 else { continue }
            requestBlockOutput(blockID: row.id_lo, span: span)
        }
    }

    private func requestBlockOutput(blockID: UInt64, span: SeyalAppBlockSpan) {
        guard span.start_line > 0 else { return }
        let end = span.end_line >= span.start_line
            ? span.end_line
            : span.start_line &+ 511
        _ = pane.inputSurface.requestHistoryRange(
            startLine: span.start_line,
            endLine: max(end, span.start_line),
            blockID: blockID
        )
    }

    private func applyHistoryRange(_ range: NativeHistoryRange) {
        pane.inputSurface.retainHistoryRange(range)
        let cellHeight = pane.inputSurface.terminalPresentationCellSize().height
        if let card = blockCards[range.blockID] {
            card.setOutputLines(max(range.rows.count, 1), cellHeight: cellHeight)
        }
        layoutSubtreeIfNeeded()
        publishBlockOutputFrame()
    }

    private func publishBlockOutputFrame() {
        let snapshot = seyal_app_snapshot(pane.appHandle)
        let direct = snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_RAW.rawValue)
            || snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_TUI.rawValue)
        guard !direct else { return }
        let surface = pane.inputSurface
        var regions: [NativeTranscriptRegion] = []
        for (blockID, card) in blockCards {
            let clip = card.body.convert(card.body.bounds, to: surface)
            guard clip.width > 0, clip.height > 0 else { continue }
            regions.append(NativeTranscriptRegion(id: blockID, origin: clip.origin, clip: clip))
        }
        regions.sort { $0.id < $1.id }
        transcriptFrameRevision &+= 1
        surface.setTranscriptFrame(
            NativeTranscriptFrame(
                revision: transcriptFrameRevision,
                regions: regions,
                surfaceIdentity: ObjectIdentifier(surface)
            )
        )
    }

    private func scrollTranscriptToLiveEnd() {
        let document = transcript.documentView ?? blocks
        let visible = transcript.contentView.bounds.height
        let height = document.fittingSize.height
        let y = max(height - visible, 0)
        transcript.contentView.scroll(to: NSPoint(x: 0, y: y))
        transcript.reflectScrolledClipView(transcript.contentView)
    }

    private func applyTheme() {
        let theme = NativeThemeRealization.theme(for: effectiveAppearance)
        NativeThemeRealization.apply(
            to: self,
            material: material,
            appearance: effectiveAppearance
        )
        applyChromeSurface(
            UInt16(SEYAL_APP_SURFACE_LEFT),
            to: left,
            theme: theme,
            cornerRadius: 0
        )
        applyChromeSurface(
            UInt16(SEYAL_APP_SURFACE_INSPECTOR),
            to: inspectorColumn,
            theme: theme,
            cornerRadius: 0
        )
        applyChromeSurface(
            UInt16(SEYAL_APP_SURFACE_TAB_STRIP),
            to: tabStrip,
            theme: theme,
            cornerRadius: 0
        )
        applyChromeSurface(
            UInt16(SEYAL_APP_SURFACE_ATTENTION_POPOVER),
            to: attentionPopover,
            theme: theme,
            cornerRadius: 8
        )
        applyChromeSurface(
            UInt16(SEYAL_APP_SURFACE_PALETTE_OVERLAY),
            to: paletteOverlay,
            theme: theme,
            cornerRadius: 10
        )
        centerColumn.layer?.backgroundColor = theme.canvas.cgColor
        transcript.backgroundColor = .clear
        left.layer?.borderWidth = 0
        composer.apply(theme: theme)
        applyComposerDepth(theme: theme)
        for view in blocks.arrangedSubviews {
            (view as? CommandBlockView)?.apply(theme: theme)
        }
    }

    private func applyChromeSurface(
        _ surface: UInt16,
        to view: NSView,
        theme: NativeTheme,
        cornerRadius: CGFloat
    ) {
        let projected = NativeThemeRealization.chromeSurface(
            surface,
            appHandle: pane.appHandle,
            appearance: effectiveAppearance
        )
        let fill = NativeThemeRealization.color(for: projected, theme: theme)
        view.wantsLayer = true
        view.layer?.backgroundColor = fill.cgColor
        if cornerRadius > 0 {
            view.layer?.cornerRadius = cornerRadius
            view.layer?.borderWidth = theme.reduceTransparency ? 1 : 0
            view.layer?.borderColor = theme.seam.cgColor
        }
        let previous = lastChromeDepths[surface]
        lastChromeDepths[surface] = projected.depth
        if !theme.reduceMotion,
           projected.depth >= UInt16(SEYAL_APP_DEPTH_ACTIVE),
           previous != projected.depth
        {
            // Short opacity settle only; never animate terminal content.
            let animation = CABasicAnimation(keyPath: "opacity")
            animation.fromValue = 0.92
            animation.toValue = 1
            animation.duration = theme.focusDuration
            view.layer?.add(animation, forKey: "seyal-depth-focus")
        }
    }

    private func applyComposerDepth(theme: NativeTheme) {
        let projected = NativeThemeRealization.chromeSurface(
            UInt16(SEYAL_APP_SURFACE_COMPOSER),
            appHandle: pane.appHandle,
            appearance: effectiveAppearance
        )
        let fill = NativeThemeRealization.color(for: projected, theme: theme)
        composer.wantsLayer = true
        composer.layer?.backgroundColor = fill.cgColor
        composer.layer?.borderColor = projected.depth >= UInt16(SEYAL_APP_DEPTH_ACTIVE)
            ? theme.accent.withAlphaComponent(0.45).cgColor
            : theme.seam.cgColor
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

    @objc func showCoreCenter() {
        applyChromeKind(UInt16(SEYAL_APP_ACTION_SET_CENTER_SURFACE.rawValue), reserved: 0)
    }

    @objc func showAgentsCenter() {
        applyChromeKind(UInt16(SEYAL_APP_ACTION_SET_CENTER_SURFACE.rawValue), reserved: 1)
    }

    func focusPaneIdentity(lo: UInt64, hi: UInt64) {
        applyChromeKind(
            UInt16(SEYAL_APP_ACTION_FOCUS_PANE.rawValue),
            reserved: 0,
            idLo: lo,
            idHi: hi
        )
    }

    @objc func createTab() {
        applyChromeKind(UInt16(SEYAL_APP_ACTION_CREATE_TAB.rawValue), reserved: 0)
    }

    @objc func splitRight() {
        applyChromeKind(UInt16(SEYAL_APP_ACTION_SPLIT_FOCUSED.rawValue), reserved: UInt32(SEYAL_APP_SPLIT_RIGHT))
    }

    @objc func splitDown() {
        applyChromeKind(UInt16(SEYAL_APP_ACTION_SPLIT_FOCUSED.rawValue), reserved: UInt32(SEYAL_APP_SPLIT_DOWN))
    }

    @objc func closeFocusedPane() {
        let shell = seyal_app_shell(pane.appHandle)
        applyChromeKind(
            UInt16(SEYAL_APP_ACTION_CLOSE_PANE.rawValue),
            reserved: 0,
            idLo: shell.focused_pane_lo,
            idHi: shell.focused_pane_hi
        )
    }

    @objc func toggleLeftPanel() {
        let chrome = seyal_app_chrome(pane.appHandle)
        let leftOn = chrome.reserved & UInt32(SEYAL_APP_CHROME_LEFT_VISIBLE) == 0
        let inspectorOn = chrome.reserved & UInt32(SEYAL_APP_CHROME_INSPECTOR_VISIBLE) != 0
        let tabOn = chrome.reserved & UInt32(SEYAL_APP_CHROME_TAB_STRIP_VISIBLE) != 0
        var reserved: UInt32 = 0
        if leftOn { reserved |= UInt32(SEYAL_APP_CHROME_LEFT_VISIBLE) }
        if inspectorOn { reserved |= UInt32(SEYAL_APP_CHROME_INSPECTOR_VISIBLE) }
        if tabOn { reserved |= UInt32(SEYAL_APP_CHROME_TAB_STRIP_VISIBLE) }
        applyChromeKind(UInt16(SEYAL_APP_ACTION_SET_SHELL_CHROME.rawValue), reserved: reserved)
    }

    @objc func toggleInspector() {
        let chrome = seyal_app_chrome(pane.appHandle)
        let leftOn = chrome.reserved & UInt32(SEYAL_APP_CHROME_LEFT_VISIBLE) != 0
        let inspectorOn = chrome.reserved & UInt32(SEYAL_APP_CHROME_INSPECTOR_VISIBLE) == 0
        let tabOn = chrome.reserved & UInt32(SEYAL_APP_CHROME_TAB_STRIP_VISIBLE) != 0
        var reserved: UInt32 = 0
        if leftOn { reserved |= UInt32(SEYAL_APP_CHROME_LEFT_VISIBLE) }
        if inspectorOn { reserved |= UInt32(SEYAL_APP_CHROME_INSPECTOR_VISIBLE) }
        if tabOn { reserved |= UInt32(SEYAL_APP_CHROME_TAB_STRIP_VISIBLE) }
        applyChromeKind(UInt16(SEYAL_APP_ACTION_SET_SHELL_CHROME.rawValue), reserved: reserved)
    }

    @objc private func setInspectorMode(_ sender: NSButton) {
        applyChromeKind(UInt16(SEYAL_APP_ACTION_SET_INSPECTOR.rawValue), reserved: UInt32(sender.tag))
    }

    private func selectInspectorBlock(idLo: UInt64, idHi: UInt64) {
        let snapshot = seyal_app_snapshot(pane.appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_SELECT_INSPECTOR_BLOCK.rawValue)
        action.applySnapshotFence(snapshot)
        action.target_execution_lo = idLo
        action.target_execution_hi = idHi
        _ = seyal_app_apply(pane.appHandle, &action)
        reconcileChrome()
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

    @objc private func toggleAttentionPopover() {
        let chrome = seyal_app_chrome(pane.appHandle)
        let open = chrome.reserved & UInt32(SEYAL_APP_CHROME_ATTENTION_POPOVER_OPEN) == 0
        applyChromeKind(
            UInt16(SEYAL_APP_ACTION_SET_ATTENTION_POPOVER.rawValue),
            reserved: open ? 1 : 0
        )
    }

    @objc func toggleCommandPalette() {
        let chrome = seyal_app_chrome(pane.appHandle)
        let open = chrome.reserved & UInt32(SEYAL_APP_CHROME_PALETTE_OPEN) == 0
        applyChromeKind(
            UInt16(SEYAL_APP_ACTION_SET_PALETTE_OPEN.rawValue),
            reserved: open ? 1 : 0
        )
    }

    @objc private func runSelectedPaletteCommand() {
        applyChromeKind(UInt16(SEYAL_APP_ACTION_PALETTE_RUN.rawValue), reserved: 0)
    }

    private func setPaletteQuery(_ query: String) {
        applyPayload(UInt16(SEYAL_APP_ACTION_SET_PALETTE_QUERY.rawValue), text: query)
    }

    private func movePalette(delta: Int32) {
        applyChromeKind(
            UInt16(SEYAL_APP_ACTION_PALETTE_MOVE.rawValue),
            reserved: UInt32(bitPattern: delta)
        )
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        let chrome = seyal_app_chrome(pane.appHandle)
        let open = chrome.reserved & UInt32(SEYAL_APP_CHROME_PALETTE_OPEN) != 0
        guard open else {
            return super.performKeyEquivalent(with: event)
        }
        if event.keyCode == 53 { // Escape
            applyChromeKind(UInt16(SEYAL_APP_ACTION_SET_PALETTE_OPEN.rawValue), reserved: 0)
            return true
        }
        if event.keyCode == 36 { // Return
            applyChromeKind(UInt16(SEYAL_APP_ACTION_PALETTE_RUN.rawValue), reserved: 0)
            return true
        }
        if event.keyCode == 125 { // Down
            movePalette(delta: 1)
            return true
        }
        if event.keyCode == 126 { // Up
            movePalette(delta: -1)
            return true
        }
        return super.performKeyEquivalent(with: event)
    }

    @objc private func openAttention(_ sender: NSButton) {
        applyPayload(UInt16(SEYAL_APP_ACTION_OPEN_ATTENTION.rawValue), text: sender.identifier?.rawValue ?? "")
    }

    @objc private func selectAgent(_ sender: NSButton) {
        applyPayload(UInt16(SEYAL_APP_ACTION_SELECT_AGENT.rawValue), text: sender.identifier?.rawValue ?? "")
    }

    func applySplitRatio(layoutIndex: UInt32, ratioBps: UInt16) {
        applyChromeKind(
            UInt16(SEYAL_APP_ACTION_SET_SPLIT_RATIO.rawValue),
            reserved: layoutIndex,
            idLo: UInt64(ratioBps),
            idHi: 0
        )
    }

    private func applyChromeKind(
        _ kind: UInt16,
        reserved: UInt32,
        idLo: UInt64 = 0,
        idHi: UInt64 = 0
    ) {
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = kind
        action.reserved = reserved
        action.target_execution_lo = idLo
        action.target_execution_hi = idHi
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
        action.applySnapshotFence(snapshot)
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
        styleSwitcher(coreCenterButton, identifier: "seyal-center-core", action: #selector(showCoreCenter))
        styleSwitcher(agentsCenterButton, identifier: "seyal-center-agents", action: #selector(showAgentsCenter))
        styleAction(newTabButton, identifier: "seyal-new-tab", action: #selector(createTab))
        styleAction(splitRightButton, identifier: "seyal-split-right", action: #selector(splitRight))
        styleAction(splitDownButton, identifier: "seyal-split-down", action: #selector(splitDown))
        styleAction(closePaneButton, identifier: "seyal-close-pane", action: #selector(closeFocusedPane))
        styleSwitcher(attentionBell, identifier: "seyal-attention-bell", action: #selector(toggleAttentionPopover))
        chromeActions.arrangedSubviews.forEach { $0.removeFromSuperview() }
        chromeActions.addArrangedSubview(coreCenterButton)
        chromeActions.addArrangedSubview(agentsCenterButton)
        chromeActions.addArrangedSubview(splitRightButton)
        chromeActions.addArrangedSubview(splitDownButton)
        chromeActions.addArrangedSubview(closePaneButton)
        chromeActions.addArrangedSubview(attentionBell)
    }

    private func styleAction(_ button: NSButton, identifier: String, action: Selector) {
        button.bezelStyle = .inline
        button.isBordered = false
        button.font = .systemFont(ofSize: 11, weight: .medium)
        button.setAccessibilityIdentifier(identifier)
        button.target = self
        button.action = action
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

extension ProductChromeHostView: NSTextFieldDelegate {
    func controlTextDidChange(_ obj: Notification) {
        guard obj.object as AnyObject? === paletteField, !isApplyingPaletteQuery else { return }
        setPaletteQuery(paletteField.stringValue)
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        guard control === paletteField else { return false }
        if commandSelector == #selector(NSResponder.insertNewline(_:)) {
            applyChromeKind(UInt16(SEYAL_APP_ACTION_PALETTE_RUN.rawValue), reserved: 0)
            return true
        }
        if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
            applyChromeKind(UInt16(SEYAL_APP_ACTION_SET_PALETTE_OPEN.rawValue), reserved: 0)
            return true
        }
        if commandSelector == #selector(NSResponder.moveDown(_:)) {
            movePalette(delta: 1)
            return true
        }
        if commandSelector == #selector(NSResponder.moveUp(_:)) {
            movePalette(delta: -1)
            return true
        }
        return false
    }
}

private final class TranscriptClipView: NSClipView {
    override var isFlipped: Bool { true }
}

private final class IdentityButton: NSButton {
    var kind: UInt16 = 0
    var idLo: UInt64 = 0
    var idHi: UInt64 = 0
}

private final class CommandBlockView: NSView {
    let body = NSView()
    private let header = NSView()
    private let prompt = NSTextField(labelWithString: "")
    private let command = NSTextField(labelWithString: "")
    private let status = NSTextField(labelWithString: "")
    private let seam = NSView()
    private let state: UInt16
    private let idLo: UInt64
    private let idHi: UInt64
    private var bodyHeight: NSLayoutConstraint!
    var onSelect: ((UInt64, UInt64) -> Void)?

    init(
        prompt: String,
        title: String,
        detail: String,
        state: UInt16,
        cellHeight: CGFloat,
        lines: Int,
        idLo: UInt64,
        idHi: UInt64
    ) {
        self.state = state
        self.idLo = idLo
        self.idHi = idHi
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = false
        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        self.prompt.stringValue = prompt
        self.prompt.font = .monospacedSystemFont(ofSize: 13, weight: .medium)
        self.prompt.setContentHuggingPriority(.required, for: .horizontal)
        self.prompt.translatesAutoresizingMaskIntoConstraints = false
        command.stringValue = title.isEmpty ? "command" : title
        setAccessibilityLabel(command.stringValue)
        command.font = .monospacedSystemFont(ofSize: 13, weight: .medium)
        command.lineBreakMode = .byTruncatingTail
        command.translatesAutoresizingMaskIntoConstraints = false
        status.stringValue = detail
        status.isHidden = detail.isEmpty
        status.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        status.tag = 2
        status.setContentHuggingPriority(.required, for: .horizontal)
        status.translatesAutoresizingMaskIntoConstraints = false
        seam.translatesAutoresizingMaskIntoConstraints = false
        seam.wantsLayer = true
        header.translatesAutoresizingMaskIntoConstraints = false
        body.translatesAutoresizingMaskIntoConstraints = false
        body.wantsLayer = true
        body.layer?.isOpaque = false
        body.layer?.backgroundColor = NSColor.clear.cgColor
        body.setAccessibilityElement(true)
        body.setAccessibilityRole(.group)
        header.addSubview(self.prompt)
        header.addSubview(command)
        header.addSubview(status)
        addSubview(header)
        addSubview(body)
        addSubview(seam)
        bodyHeight = body.heightAnchor.constraint(
            equalToConstant: max(cellHeight, 1) * CGFloat(max(lines, 1))
        )
        NSLayoutConstraint.activate([
            header.leadingAnchor.constraint(equalTo: leadingAnchor),
            header.trailingAnchor.constraint(equalTo: trailingAnchor),
            header.topAnchor.constraint(equalTo: topAnchor),
            header.heightAnchor.constraint(greaterThanOrEqualToConstant: 22),
            self.prompt.leadingAnchor.constraint(equalTo: header.leadingAnchor),
            self.prompt.centerYAnchor.constraint(equalTo: header.centerYAnchor),
            command.leadingAnchor.constraint(equalTo: self.prompt.trailingAnchor, constant: 8),
            command.centerYAnchor.constraint(equalTo: header.centerYAnchor),
            status.trailingAnchor.constraint(equalTo: header.trailingAnchor),
            status.centerYAnchor.constraint(equalTo: header.centerYAnchor),
            command.trailingAnchor.constraint(lessThanOrEqualTo: status.leadingAnchor, constant: -12),
            body.leadingAnchor.constraint(equalTo: command.leadingAnchor),
            body.trailingAnchor.constraint(equalTo: trailingAnchor),
            body.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 4),
            bodyHeight,
            seam.leadingAnchor.constraint(equalTo: leadingAnchor),
            seam.trailingAnchor.constraint(equalTo: trailingAnchor),
            seam.topAnchor.constraint(equalTo: body.bottomAnchor, constant: 12),
            seam.bottomAnchor.constraint(equalTo: bottomAnchor),
            seam.heightAnchor.constraint(equalToConstant: 1),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("CommandBlockView is programmatic")
    }

    override func mouseDown(with event: NSEvent) {
        if idLo != 0 || idHi != 0 {
            onSelect?(idLo, idHi)
        }
        super.mouseDown(with: event)
    }

    func setOutputLines(_ lines: Int, cellHeight: CGFloat) {
        bodyHeight.constant = max(cellHeight, 1) * CGFloat(max(lines, 1))
    }

    func apply(theme: NativeTheme) {
        body.layer?.isOpaque = false
        body.layer?.backgroundColor = NSColor.clear.cgColor
        prompt.textColor = theme.accent
        command.textColor = theme.accent
        if state == UInt16(SEYAL_APP_BLOCK_STATE_FAILED) {
            status.textColor = theme.danger
            seam.layer?.backgroundColor = theme.danger.withAlphaComponent(0.45).cgColor
        } else {
            status.textColor = theme.muted
            seam.layer?.backgroundColor = theme.seam.cgColor
        }
    }
}

private func copyUTF8(_ pointer: UnsafePointer<UInt8>?, _ length: UInt32) -> String? {
    guard length > 0, let pointer else { return nil }
    return String(decoding: UnsafeBufferPointer(start: pointer, count: Int(length)), as: UTF8.self)
}
