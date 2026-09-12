import AppKit

/// New thin AppKit host. Product decisions come from Rust snapshots/actions.
@MainActor
final class SeyalThinHostView: NSView, NSTextViewDelegate {
    private let product = SeyalProductBridge()
    private let chrome = NSTextField(labelWithString: "")
    private let editor = NSTextView()
    private var applyingSnapshot = false
    private var terminal: MetalSurfaceView?

    init(frame: NSRect, terminalFont: SeyalResolvedFontSpec) {
        super.init(frame: frame)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true

        chrome.translatesAutoresizingMaskIntoConstraints = false
        chrome.font = .systemFont(ofSize: 12)
        chrome.setAccessibilityIdentifier("seyal-thin-host-chrome")

        editor.translatesAutoresizingMaskIntoConstraints = false
        editor.isRichText = false
        editor.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        editor.delegate = self
        editor.setAccessibilityIdentifier("seyal-thin-host-composer")

        let terminal = MetalSurfaceView(
            frame: .zero,
            paneID: "thin-host",
            terminalFont: terminalFont
        )
        terminal.translatesAutoresizingMaskIntoConstraints = false
        self.terminal = terminal

        addSubview(chrome)
        addSubview(terminal)
        addSubview(editor)
        NSLayoutConstraint.activate([
            chrome.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            chrome.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            chrome.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            terminal.leadingAnchor.constraint(equalTo: leadingAnchor),
            terminal.trailingAnchor.constraint(equalTo: trailingAnchor),
            terminal.topAnchor.constraint(equalTo: chrome.bottomAnchor, constant: 8),
            editor.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            editor.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            editor.topAnchor.constraint(equalTo: terminal.bottomAnchor, constant: 8),
            editor.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -12),
            editor.heightAnchor.constraint(equalToConstant: 56),
        ])
        reconcile()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("SeyalThinHostView is programmatic")
    }

    func textDidChange(_ notification: Notification) {
        guard !applyingSnapshot else { return }
        product.setDraft(editor.string)
        reconcile()
    }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 36, !event.modifierFlags.contains(.shift) {
            product.submit()
            reconcile()
            return
        }
        super.keyDown(with: event)
    }

    private func reconcile() {
        let snap = product.snapshot()
        applyingSnapshot = true
        chrome.stringValue = "\(snap.workspaceName) / \(snap.tabTitle) / \(snap.paneTitle)"
        if editor.string != snap.draft {
            editor.string = snap.draft
        }
        editor.isEditable = !snap.composerBusy && !snap.composerHidden
        editor.isHidden = snap.composerHidden
        applyingSnapshot = false
    }
}
