import AppKit
import QuartzCore

/// IME/first-responder adapter. Presentation eligibility is read from Rust.
@MainActor
final class InteractiveMetalSurfaceView: MetalSurfaceView, @preconcurrency NSTextInputClient {
    private let appHandle: UInt64
    private var marked = ""
    var onBridgeBecameUsable: (() -> Void)?
    var observedAlternateScreen = false

    init(frame frameRect: NSRect, appHandle: UInt64) {
        self.appHandle = appHandle
        super.init(frame: frameRect, paneID: "m001-pane")
        wantsLayer = true
        setAccessibilityIdentifier("terminal-input")
        setAccessibilityRole(.textArea)
        setAccessibilityElement(true)
    }

    override func restoreNativeInteractionAfterRendererReady() -> Bool {
        window?.makeFirstResponder(self) ?? false
    }

    override func terminalBridgeStatusDidChange() {
        super.terminalBridgeStatusDidChange()
        if terminalBridgeIsConnected {
            onBridgeBecameUsable?()
        }
    }

    override var acceptsFirstResponder: Bool { true }

    override func becomeFirstResponder() -> Bool {
        let became = super.becomeFirstResponder()
        if became {
            inputContext?.activate()
        }
        return became
    }

    override func keyDown(with event: NSEvent) {
        guard allowsDirectTerminalInput else { return }
        inputContext?.handleEvent(event)
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(self)
        super.mouseDown(with: event)
    }

    func hasMarkedText() -> Bool { !marked.isEmpty }

    func markedRange() -> NSRange {
        marked.isEmpty ? NSRange(location: NSNotFound, length: 0) : NSRange(location: 0, length: marked.utf16.count)
    }

    func selectedRange() -> NSRange {
        NSRange(location: marked.utf16.count, length: 0)
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        marked = text(from: string) ?? ""
    }

    func unmarkText() {
        let committed = marked
        marked = ""
        submitIfAllowed(committed)
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        nil
    }

    func insertText(_ string: Any, replacementRange: NSRange) {
        marked = ""
        submitIfAllowed(text(from: string) ?? "")
    }

    func characterIndex(for point: NSPoint) -> Int { 0 }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        window?.convertToScreen(convert(bounds, to: nil)) ?? .zero
    }

    override func doCommand(by selector: Selector) {
        guard allowsDirectTerminalInput else { return }
        switch selector {
        case #selector(insertNewline(_:)):
            _ = terminalSubmitKey(kind: UInt16(SEYAL_KEY_ENTER.rawValue), scalar: 0)
        case #selector(deleteBackward(_:)):
            _ = terminalSubmitKey(kind: UInt16(SEYAL_KEY_BACKSPACE.rawValue), scalar: 0)
        case #selector(insertTab(_:)):
            _ = terminalSubmitKey(kind: UInt16(SEYAL_KEY_TAB.rawValue), scalar: 0)
        case #selector(cancelOperation(_:)):
            _ = terminalSubmitKey(kind: UInt16(SEYAL_KEY_ESCAPE.rawValue), scalar: 0)
        case #selector(moveUp(_:)):
            _ = terminalSubmitKey(kind: UInt16(SEYAL_KEY_ARROW_UP.rawValue), scalar: 0)
        case #selector(moveDown(_:)):
            _ = terminalSubmitKey(kind: UInt16(SEYAL_KEY_ARROW_DOWN.rawValue), scalar: 0)
        case #selector(moveLeft(_:)):
            _ = terminalSubmitKey(kind: UInt16(SEYAL_KEY_ARROW_LEFT.rawValue), scalar: 0)
        case #selector(moveRight(_:)):
            _ = terminalSubmitKey(kind: UInt16(SEYAL_KEY_ARROW_RIGHT.rawValue), scalar: 0)
        default:
            break
        }
    }

    func copy(_ sender: Any?) {
        let snapshot = seyal_app_snapshot(appHandle)
        guard snapshot.output_utf8_len > 0, let bytes = snapshot.output_utf8 else { return }
        let text = String(
            decoding: UnsafeBufferPointer(start: bytes, count: Int(snapshot.output_utf8_len)),
            as: UTF8.self
        )
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
    }

    func paste(_ sender: Any?) {
        guard allowsDirectTerminalInput else { return }
        if let text = NSPasteboard.general.string(forType: .string) {
            submitIfAllowed(text)
        }
    }

    private var allowsDirectTerminalInput: Bool {
        let eligibility = seyal_app_snapshot(appHandle).eligibility
        return eligibility == UInt16(SEYAL_APP_ELIGIBILITY_RAW.rawValue)
            || eligibility == UInt16(SEYAL_APP_ELIGIBILITY_TUI.rawValue)
    }

    private func submitIfAllowed(_ text: String) {
        guard allowsDirectTerminalInput, !text.isEmpty else { return }
        _ = terminalSubmitCommittedText(text)
    }

    private func text(from string: Any) -> String? {
        if let text = string as? String { return text }
        if let attributed = string as? NSAttributedString { return attributed.string }
        return nil
    }
}
