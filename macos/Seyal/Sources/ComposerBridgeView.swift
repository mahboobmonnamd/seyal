import AppKit

/// Native IME/editor bridge only. Draft and submit stay Rust-owned.
@MainActor
final class ComposerBridgeView: NSView, NSTextViewDelegate {
    private let appHandle: UInt64
    private let textView = NSTextView()
    private var lastEpoch: UInt64 = 0

    init(appHandle: UInt64) {
        self.appHandle = appHandle
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        setAccessibilityIdentifier("seyal-composer")
        setAccessibilityRole(.textArea)
        textView.delegate = self
        textView.isRichText = false
        textView.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        textView.translatesAutoresizingMaskIntoConstraints = false
        addSubview(textView)
        NSLayoutConstraint.activate([
            textView.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            textView.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
            textView.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            textView.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
            heightAnchor.constraint(greaterThanOrEqualToConstant: 44),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("ComposerBridgeView is programmatic")
    }

    func reconcile() {
        let composer = seyal_app_composer(appHandle)
        let hidden = composer.mode == UInt16(SEYAL_APP_COMPOSER_HIDDEN.rawValue)
        isHidden = hidden
        textView.isEditable = composer.mode == UInt16(SEYAL_APP_COMPOSER_AVAILABLE.rawValue)
        guard composer.epoch != lastEpoch else { return }
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

    func textDidChange(_ notification: Notification) {
        pushDraft()
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
        let snapshot = seyal_app_snapshot(appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_SUBMIT_COMPOSER.rawValue)
        action.fence_pane_lo = snapshot.pane_lo
        action.fence_pane_hi = snapshot.pane_hi
        action.fence_epoch = snapshot.epoch
        action.target_pty_generation = composer.epoch
        _ = seyal_app_apply(appHandle, &action)
        reconcile()
    }
}
