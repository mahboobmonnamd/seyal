import AppKit

/// Native IME/editor bridge only. Draft, submit, history overlay, and C09 copy stay Rust-owned.
@MainActor
final class ComposerBridgeView: NSView, NSTextViewDelegate, NSTextFieldDelegate {
    var onSubmitComposer: ((String) -> Int32)?
    var onSubmitRaw: ((String) -> Int32)?

    private let appHandle: UInt64
    private let textView = NSTextView()
    private let placeholder = NSTextField(labelWithString: "")
    private let execute = NSButton(title: "", target: nil, action: nil)
    private let historyPanel = NSStackView()
    private let historyFilter = NSTextField(string: "")
    private let historyRows = NSStackView()
    private var heightConstraint: NSLayoutConstraint!
    private var lastEpoch: UInt64 = 0
    private var theme: NativeTheme?
    private var editing = false
    private var historyHighlight = 0

    init(appHandle: UInt64) {
        self.appHandle = appHandle
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true
        layer?.cornerRadius = 10
        layer?.cornerCurve = .continuous
        layer?.masksToBounds = true
        focusRingType = .none
        setAccessibilityIdentifier("seyal-composer")
        // Group, not a leaf textArea: XCUI must see both the dock and the
        // execute control. A leaf role hid `seyal-composer-execute`.
        setAccessibilityRole(.group)
        setAccessibilityElement(true)

        placeholder.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        placeholder.tag = 2
        placeholder.setAccessibilityElement(false)

        execute.target = self
        execute.action = #selector(executeClicked)
        execute.isBordered = false
        execute.font = .systemFont(ofSize: 13, weight: .medium)
        execute.focusRingType = .none
        execute.setButtonType(.momentaryPushIn)
        execute.setContentHuggingPriority(.required, for: .horizontal)
        execute.setContentCompressionResistancePriority(.required, for: .horizontal)
        execute.setAccessibilityIdentifier("seyal-composer-execute")
        execute.setAccessibilityRole(.button)

        textView.delegate = self
        textView.isRichText = false
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        textView.drawsBackground = false
        textView.focusRingType = .none
        textView.isVerticallyResizable = true
        textView.textContainer?.lineFragmentPadding = 0
        textView.textContainer?.widthTracksTextView = true
        textView.translatesAutoresizingMaskIntoConstraints = false
        textView.setAccessibilityElement(true)
        textView.setAccessibilityIdentifier("seyal-composer-editor")

        historyPanel.orientation = .vertical
        historyPanel.alignment = .leading
        historyPanel.spacing = 6
        historyPanel.edgeInsets = NSEdgeInsets(top: 8, left: 10, bottom: 4, right: 10)
        historyPanel.translatesAutoresizingMaskIntoConstraints = false
        historyPanel.isHidden = true
        historyPanel.setAccessibilityIdentifier("seyal-composer-history")
        historyPanel.setAccessibilityElement(true)
        historyPanel.setAccessibilityRole(.group)

        historyFilter.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        historyFilter.placeholderString = "Filter history"
        historyFilter.isBordered = true
        historyFilter.isBezeled = true
        historyFilter.bezelStyle = .roundedBezel
        historyFilter.focusRingType = .none
        historyFilter.delegate = self
        historyFilter.setAccessibilityIdentifier("seyal-composer-history-filter")
        historyFilter.translatesAutoresizingMaskIntoConstraints = false

        historyRows.orientation = .vertical
        historyRows.alignment = .leading
        historyRows.spacing = 2
        historyRows.translatesAutoresizingMaskIntoConstraints = false

        historyPanel.addArrangedSubview(historyFilter)
        historyPanel.addArrangedSubview(historyRows)

        addSubview(historyPanel)
        addSubview(textView)
        addSubview(placeholder)
        addSubview(execute)
        placeholder.translatesAutoresizingMaskIntoConstraints = false
        execute.translatesAutoresizingMaskIntoConstraints = false
        heightConstraint = heightAnchor.constraint(equalToConstant: 40)
        NSLayoutConstraint.activate([
            heightConstraint,
            historyPanel.leadingAnchor.constraint(equalTo: leadingAnchor),
            historyPanel.trailingAnchor.constraint(equalTo: trailingAnchor),
            historyPanel.topAnchor.constraint(equalTo: topAnchor),
            textView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 14),
            textView.trailingAnchor.constraint(equalTo: execute.leadingAnchor, constant: -8),
            textView.topAnchor.constraint(equalTo: historyPanel.bottomAnchor, constant: 8),
            textView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -8),
            placeholder.leadingAnchor.constraint(equalTo: textView.leadingAnchor),
            placeholder.centerYAnchor.constraint(equalTo: textView.centerYAnchor),
            execute.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            execute.centerYAnchor.constraint(equalTo: textView.centerYAnchor),
            execute.widthAnchor.constraint(greaterThanOrEqualToConstant: 28),
            execute.heightAnchor.constraint(equalToConstant: 28),
            historyFilter.widthAnchor.constraint(equalTo: historyPanel.widthAnchor, constant: -20),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("ComposerBridgeView is programmatic")
    }

    func focusEditor() {
        window?.makeFirstResponder(textView)
    }

    func apply(theme: NativeTheme) {
        self.theme = theme
        placeholder.textColor = theme.muted
        textView.textColor = theme.text
        textView.insertionPointColor = theme.accent
        historyFilter.textColor = theme.text
        paintChrome()
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if event.modifierFlags.contains(.control),
           event.charactersIgnoringModifiers?.lowercased() == "r"
        {
            toggleHistory()
            return true
        }
        return super.performKeyEquivalent(with: event)
    }

    func reconcile() {
        let composer = seyal_app_composer(appHandle)
        let snapshot = seyal_app_snapshot(appHandle)
        let direct = snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_RAW.rawValue)
            || snapshot.eligibility == UInt16(SEYAL_APP_ELIGIBILITY_TUI.rawValue)
        isHidden = direct
        let available = composer.mode == UInt16(SEYAL_APP_COMPOSER_AVAILABLE.rawValue)
        let busy = composer.mode == UInt16(SEYAL_APP_COMPOSER_BUSY.rawValue)
        textView.isEditable = available
        execute.title = copyString(UInt16(SEYAL_APP_COPY_COMPOSER_EXECUTE))
        execute.isEnabled = composer.flags & UInt16(SEYAL_APP_COMPOSER_CAN_SUBMIT) != 0
        placeholder.stringValue = copyString(UInt16(SEYAL_APP_COPY_COMPOSER_PLACEHOLDER))
        setAccessibilityValue(available ? "available" : (busy ? "busy" : "hidden"))
        if composer.epoch != lastEpoch {
            lastEpoch = composer.epoch
            if composer.draft_utf8_len > 0, let bytes = composer.draft_utf8 {
                textView.string = String(
                    decoding: UnsafeBufferPointer(start: bytes, count: Int(composer.draft_utf8_len)),
                    as: UTF8.self
                )
            } else if composer.mode != UInt16(SEYAL_APP_COMPOSER_BUSY.rawValue) {
                textView.string = ""
            }
        }
        placeholder.isHidden = !textView.string.isEmpty
        rebuildHistory(composer)
        expandToDraft()
        paintChrome()
    }

    func textDidBeginEditing(_ notification: Notification) {
        editing = true
        paintChrome()
    }

    func textDidEndEditing(_ notification: Notification) {
        editing = false
        paintChrome()
    }

    func textDidChange(_ notification: Notification) {
        placeholder.isHidden = !textView.string.isEmpty
        pushDraft()
        let composer = seyal_app_composer(appHandle)
        execute.isEnabled = composer.flags & UInt16(SEYAL_APP_COMPOSER_CAN_SUBMIT) != 0
        expandToDraft()
    }

    func controlTextDidChange(_ obj: Notification) {
        pushHistoryQuery(historyFilter.stringValue)
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(NSResponder.cancelOperation(_:)) {
            dismissHistory()
            return true
        }
        if commandSelector == #selector(NSResponder.moveUp(_:)) {
            historyHighlight = max(0, historyHighlight - 1)
            rebuildHistory(seyal_app_composer(appHandle))
            return true
        }
        if commandSelector == #selector(NSResponder.moveDown(_:)) {
            let count = Int(seyal_app_composer(appHandle).history_match_count)
            if count > 0 {
                historyHighlight = min(count - 1, historyHighlight + 1)
                rebuildHistory(seyal_app_composer(appHandle))
            }
            return true
        }
        if commandSelector == #selector(NSResponder.insertNewline(_:)) {
            selectHighlightedHistory()
            return true
        }
        return false
    }

    func textView(_ textView: NSTextView, shouldChangeTextIn affectedCharRange: NSRange, replacementString: String?) -> Bool {
        if replacementString == "\n" || replacementString == "\r" {
            if shiftHeld() {
                return true
            }
            submit()
            return false
        }
        return true
    }

    func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(NSResponder.insertNewline(_:)) {
            if shiftHeld() {
                return false
            }
            submit()
            return true
        }
        return false
    }

    @objc private func executeClicked() {
        submit()
    }

    @objc private func historyRowClicked(_ sender: NSButton) {
        selectHistory(index: UInt32(sender.tag))
    }

    private func shiftHeld() -> Bool {
        NSApp.currentEvent?.modifierFlags.contains(.shift) == true
    }

    private func toggleHistory() {
        let composer = seyal_app_composer(appHandle)
        if composer.flags & UInt16(SEYAL_APP_COMPOSER_HISTORY_OPEN) != 0 {
            dismissHistory()
        } else {
            openHistory()
        }
    }

    private func openHistory() {
        applyKind(UInt16(SEYAL_APP_ACTION_OPEN_COMPOSER_HISTORY.rawValue))
        historyHighlight = 0
        historyFilter.stringValue = ""
        reconcile()
        window?.makeFirstResponder(historyFilter)
    }

    private func dismissHistory() {
        applyKind(UInt16(SEYAL_APP_ACTION_DISMISS_COMPOSER_HISTORY.rawValue))
        reconcile()
        focusEditor()
    }

    private func pushHistoryQuery(_ query: String) {
        let snapshot = seyal_app_snapshot(appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_SET_COMPOSER_HISTORY_QUERY.rawValue)
        action.applySnapshotFence(snapshot)
        let utf8 = Array(query.utf8)
        utf8.withUnsafeBufferPointer { buffer in
            action.payload = buffer.baseAddress
            action.payload_len = UInt32(buffer.count)
            _ = seyal_app_apply(appHandle, &action)
        }
        historyHighlight = 0
        reconcile()
    }

    private func selectHighlightedHistory() {
        let count = Int(seyal_app_composer(appHandle).history_match_count)
        guard count > 0 else { return }
        selectHistory(index: UInt32(min(historyHighlight, count - 1)))
    }

    private func selectHistory(index: UInt32) {
        let composer = seyal_app_composer(appHandle)
        let snapshot = seyal_app_snapshot(appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_SELECT_COMPOSER_HISTORY.rawValue)
        action.applySnapshotFence(snapshot)
        action.reserved = index
        action.target_pty_generation = composer.epoch
        _ = seyal_app_apply(appHandle, &action)
        reconcile()
        focusEditor()
    }

    private func rebuildHistory(_ composer: SeyalAppComposer) {
        let open = composer.flags & UInt16(SEYAL_APP_COMPOSER_HISTORY_OPEN) != 0
        historyPanel.isHidden = !open
        historyPanel.setAccessibilityElement(open)
        historyRows.arrangedSubviews.forEach { $0.removeFromSuperview() }
        guard open else { return }
        if composer.history_query_utf8_len > 0, let bytes = composer.history_query_utf8 {
            let query = String(
                decoding: UnsafeBufferPointer(start: bytes, count: Int(composer.history_query_utf8_len)),
                as: UTF8.self
            )
            if historyFilter.stringValue != query {
                historyFilter.stringValue = query
            }
        } else if !historyFilter.stringValue.isEmpty, historyFilter.currentEditor() == nil {
            historyFilter.stringValue = ""
        }
        let count = Int(composer.history_match_count)
        if count == 0 {
            let empty = NSTextField(labelWithString: "No matching history")
            empty.font = .systemFont(ofSize: 12, weight: .regular)
            empty.setAccessibilityIdentifier("seyal-composer-history-empty")
            historyRows.addArrangedSubview(empty)
            return
        }
        historyHighlight = min(historyHighlight, count - 1)
        for index in 0..<count {
            let row = seyal_app_history_row(appHandle, UInt32(index))
            let title = copyUTF8(row.title, row.title_len) ?? ""
            let button = NSButton(title: title, target: self, action: #selector(historyRowClicked(_:)))
            button.isBordered = false
            button.alignment = .left
            button.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
            button.tag = index
            button.setAccessibilityIdentifier("seyal-composer-history-\(index)")
            if index == historyHighlight, let theme {
                button.contentTintColor = theme.accent
            } else if let theme {
                button.contentTintColor = theme.text
            }
            historyRows.addArrangedSubview(button)
        }
    }

    private func paintChrome() {
        guard let theme else { return }
        layer?.backgroundColor = theme.elevated.cgColor
        layer?.borderWidth = 1
        layer?.borderColor = editing
            ? theme.accent.withAlphaComponent(0.45).cgColor
            : theme.seam.cgColor
        execute.contentTintColor = theme.muted
        execute.attributedTitle = NSAttributedString(
            string: execute.title,
            attributes: [
                .font: NSFont.systemFont(ofSize: 13, weight: .medium),
                .foregroundColor: execute.isEnabled ? theme.accent : theme.muted,
            ]
        )
    }

    private func expandToDraft() {
        guard let container = textView.textContainer,
              let layout = textView.layoutManager
        else { return }
        layout.ensureLayout(for: container)
        let used = layout.usedRect(for: container).height
        let historyExtra: CGFloat = historyPanel.isHidden ? 0 : 120
        heightConstraint.constant = min(max(40, used + 16 + historyExtra), 240)
    }

    private func copyString(_ kind: UInt16) -> String {
        let row = seyal_app_copy(appHandle, kind)
        return copyUTF8(row.title, row.title_len) ?? ""
    }

    private func applyKind(_ kind: UInt16) {
        let snapshot = seyal_app_snapshot(appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = kind
        action.applySnapshotFence(snapshot)
        _ = seyal_app_apply(appHandle, &action)
    }

    private func pushDraft() {
        let composer = seyal_app_composer(appHandle)
        let snapshot = seyal_app_snapshot(appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_SET_COMPOSER_DRAFT.rawValue)
        action.applySnapshotFence(snapshot)
        action.target_pty_generation = composer.epoch
        let utf8 = Array(textView.string.utf8)
        utf8.withUnsafeBufferPointer { buffer in
            action.payload = buffer.baseAddress
            action.payload_len = UInt32(buffer.count)
            _ = seyal_app_apply(appHandle, &action)
        }
    }

    private func submit() {
        pushDraft()
        let composer = seyal_app_composer(appHandle)
        guard composer.flags & UInt16(SEYAL_APP_COMPOSER_CAN_SUBMIT) != 0 else { return }
        let command = textView.string
        let snapshot = seyal_app_snapshot(appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_SUBMIT_COMPOSER.rawValue)
        action.applySnapshotFence(snapshot)
        action.target_pty_generation = composer.epoch
        guard seyal_app_apply(appHandle, &action) == 0 else { return }
        var status = onSubmitComposer?(command) ?? -10
        if status == -12 {
            let payload = command.hasSuffix("\r") ? command : command + "\r"
            status = onSubmitRaw?(payload) ?? -10
        }
        if status == 0 {
            let submitted = seyal_app_composer(appHandle)
            if submitted.request_id != 0 {
                applyComposerResult(requestID: submitted.request_id, accepted: true)
            }
        }
        reconcile()
    }

    func applyComposerResult(requestID: UInt64, accepted: Bool) {
        let snapshot = seyal_app_snapshot(appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_APPLY_COMPOSER_RESULT.rawValue)
        action.applySnapshotFence(snapshot)
        action.target_execution_lo = requestID
        action.reserved = accepted ? 1 : 0
        _ = seyal_app_apply(appHandle, &action)
        reconcile()
    }
}

private func copyUTF8(_ pointer: UnsafePointer<UInt8>?, _ length: UInt32) -> String? {
    guard length > 0, let pointer else { return nil }
    return String(decoding: UnsafeBufferPointer(start: pointer, count: Int(length)), as: UTF8.self)
}
