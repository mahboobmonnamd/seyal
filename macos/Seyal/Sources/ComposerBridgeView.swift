import AppKit

/// Native IME/editor bridge only. Draft and submit stay Rust-owned.
@MainActor
final class ComposerBridgeView: NSView, NSTextViewDelegate {
    var onSubmitComposer: ((String) -> Int32)?
    var onSubmitRaw: ((String) -> Int32)?

    private let appHandle: UInt64
    private let prompt = NSTextField(labelWithString: "❯")
    private let textView = NSTextView()
    private let placeholder = NSTextField(labelWithString: "command")
    private let hint = NSTextField(labelWithString: "enter")
    private var lastEpoch: UInt64 = 0

    init(appHandle: UInt64) {
        self.appHandle = appHandle
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true
        layer?.cornerRadius = 6
        setAccessibilityIdentifier("seyal-composer")
        setAccessibilityRole(.textArea)
        setAccessibilityElement(true)

        prompt.font = .monospacedSystemFont(ofSize: 13, weight: .medium)
        prompt.setContentHuggingPriority(.required, for: .horizontal)
        prompt.setAccessibilityElement(false)

        hint.font = .systemFont(ofSize: 11, weight: .medium)
        hint.tag = 2
        hint.setContentHuggingPriority(.required, for: .horizontal)
        hint.setAccessibilityElement(false)

        placeholder.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        placeholder.tag = 2
        placeholder.setAccessibilityElement(false)

        textView.delegate = self
        textView.isRichText = false
        textView.isAutomaticQuoteSubstitutionEnabled = false
        textView.isAutomaticDashSubstitutionEnabled = false
        textView.isAutomaticTextReplacementEnabled = false
        textView.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        textView.drawsBackground = false
        textView.translatesAutoresizingMaskIntoConstraints = false
        textView.setAccessibilityElement(true)
        textView.setAccessibilityIdentifier("seyal-composer-editor")

        addSubview(prompt)
        addSubview(textView)
        addSubview(placeholder)
        addSubview(hint)
        prompt.translatesAutoresizingMaskIntoConstraints = false
        placeholder.translatesAutoresizingMaskIntoConstraints = false
        hint.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            heightAnchor.constraint(greaterThanOrEqualToConstant: 52),
            prompt.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            prompt.centerYAnchor.constraint(equalTo: centerYAnchor),
            textView.leadingAnchor.constraint(equalTo: prompt.trailingAnchor, constant: 8),
            textView.trailingAnchor.constraint(equalTo: hint.leadingAnchor, constant: -8),
            textView.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            textView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -8),
            placeholder.leadingAnchor.constraint(equalTo: textView.leadingAnchor),
            placeholder.centerYAnchor.constraint(equalTo: textView.centerYAnchor),
            hint.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            hint.centerYAnchor.constraint(equalTo: centerYAnchor),
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
        layer?.backgroundColor = theme.elevated.cgColor
        layer?.borderWidth = 1
        layer?.borderColor = theme.seam.cgColor
        prompt.textColor = theme.accent
        placeholder.textColor = theme.muted
        hint.textColor = theme.muted
        textView.textColor = theme.text
        textView.insertionPointColor = theme.accent
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
        hint.stringValue = available ? "enter" : (busy ? "busy" : "")
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
    }

    func textDidChange(_ notification: Notification) {
        placeholder.isHidden = !textView.string.isEmpty
        pushDraft()
    }

    func textView(_ textView: NSTextView, shouldChangeTextIn affectedCharRange: NSRange, replacementString: String?) -> Bool {
        if replacementString == "\n" || replacementString == "\r" {
            submit()
            return false
        }
        return true
    }

    func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(NSResponder.insertNewline(_:)) {
            submit()
            return true
        }
        return false
    }

    private func pushDraft() {
        let composer = seyal_app_composer(appHandle)
        let snapshot = seyal_app_snapshot(appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_SET_COMPOSER_DRAFT.rawValue)
        action.fence_pane_lo = snapshot.pane_lo
        action.fence_pane_hi = snapshot.pane_hi
        action.fence_epoch = snapshot.epoch
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
        action.fence_pane_lo = snapshot.pane_lo
        action.fence_pane_hi = snapshot.pane_hi
        action.fence_epoch = snapshot.epoch
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
        action.fence_pane_lo = snapshot.pane_lo
        action.fence_pane_hi = snapshot.pane_hi
        action.fence_epoch = snapshot.epoch
        action.target_execution_lo = requestID
        action.reserved = accepted ? 1 : 0
        _ = seyal_app_apply(appHandle, &action)
        reconcile()
    }
}
