import AppKit
import QuartzCore

private let maxCompositionUTF8Bytes = 65_536

private enum TerminalKeyIntent: UInt16 {
    case enter = 1
    case tab = 2
    case backspace = 3
    case escape = 4
    case arrowUp = 5
    case arrowDown = 6
    case arrowRight = 7
    case arrowLeft = 8
    case controlASCII = 9
}

struct TerminalNativeKeyV2: Equatable {
    let kind: UInt16
    let modifiers: UInt16
    let value: UInt32
    let shiftedASCII: UInt32
}

private enum CompositionMutationError: Error, Equatable {
    case invalidRange
    case tooLarge
}

private struct CompositionDocument: Equatable {
    private(set) var text = ""
    private(set) var selection = NSRange(location: 0, length: 0)

    var utf16Length: Int { (text as NSString).length }

    var hasMarkedText: Bool { utf16Length > 0 }

    var markedRange: NSRange {
        hasMarkedText
            ? NSRange(location: 0, length: utf16Length)
            : NSRange(location: NSNotFound, length: 0)
    }

    var selectedRange: NSRange {
        hasMarkedText ? selection : NSRange(location: 0, length: 0)
    }

    mutating func clear() {
        text = ""
        selection = NSRange(location: 0, length: 0)
    }

    mutating func setMarkedText(
        _ inserted: String,
        selectedRange insertedSelection: NSRange,
        replacementRange requestedReplacement: NSRange
    ) throws {
        let currentLength = utf16Length
        let replacement: NSRange
        if requestedReplacement.location == NSNotFound {
            guard requestedReplacement.length == 0 else {
                throw CompositionMutationError.invalidRange
            }
            replacement = selectedRange
        } else {
            guard let valid = Self.validatedRange(requestedReplacement, upperBound: currentLength) else {
                throw CompositionMutationError.invalidRange
            }
            replacement = valid
        }

        let insertedLength = (inserted as NSString).length
        guard Self.validatedRange(insertedSelection, upperBound: insertedLength) != nil else {
            throw CompositionMutationError.invalidRange
        }

        let mutable = NSMutableString(string: text)
        mutable.replaceCharacters(in: replacement, with: inserted)
        let candidate = mutable as String
        guard candidate.utf8.count <= maxCompositionUTF8Bytes else {
            throw CompositionMutationError.tooLarge
        }

        let (absoluteLocation, overflow) = replacement.location.addingReportingOverflow(
            insertedSelection.location
        )
        guard !overflow else {
            throw CompositionMutationError.invalidRange
        }
        let absoluteSelection = NSRange(location: absoluteLocation, length: insertedSelection.length)
        guard Self.validatedRange(absoluteSelection, upperBound: (candidate as NSString).length) != nil else {
            throw CompositionMutationError.invalidRange
        }

        text = candidate
        selection = absoluteSelection
    }

    func validatesReplacementRange(_ range: NSRange) -> Bool {
        if range.location == NSNotFound {
            return range.length == 0
        }
        return Self.validatedRange(range, upperBound: utf16Length) != nil
    }

    func attributedSubstring(for proposedRange: NSRange) -> (NSAttributedString, NSRange)? {
        guard proposedRange.location != NSNotFound else { return nil }
        let length = utf16Length
        guard let proposedEnd = Self.checkedEnd(proposedRange) else { return nil }
        if proposedRange.location > length
            || (proposedRange.location == length && proposedRange.length > 0)
        {
            return nil
        }
        let boundedStart = min(proposedRange.location, length)
        let boundedEnd = min(proposedEnd, length)
        let bounded = NSRange(location: boundedStart, length: boundedEnd - boundedStart)
        if bounded.length == 0 {
            return (NSAttributedString(string: ""), bounded)
        }
        let storage = text as NSString
        let composed = storage.rangeOfComposedCharacterSequences(for: bounded)
        guard let valid = Self.validatedRange(composed, upperBound: length) else { return nil }
        return (NSAttributedString(string: storage.substring(with: valid)), valid)
    }

    func validatedCoordinateRange(_ range: NSRange) -> NSRange? {
        guard range.location != NSNotFound else { return nil }
        let length = utf16Length
        guard let end = Self.checkedEnd(range), range.location <= length else { return nil }
        if range.length == 0 {
            return NSRange(location: range.location, length: 0)
        }
        if range.location == length {
            return nil
        }
        let bounded = NSRange(location: range.location, length: min(end, length) - range.location)
        return (text as NSString).rangeOfComposedCharacterSequences(for: bounded)
    }

    private static func validatedRange(_ range: NSRange, upperBound: Int) -> NSRange? {
        guard range.location != NSNotFound,
            range.location <= upperBound,
            let end = checkedEnd(range),
            end <= upperBound
        else {
            return nil
        }
        return range
    }

    private static func checkedEnd(_ range: NSRange) -> Int? {
        let (end, overflow) = range.location.addingReportingOverflow(range.length)
        return overflow ? nil : end
    }
}

enum TerminalNativeKeyClassifier {
    /// Hardware key-code → V2 function index. Immutable; not rebuilt per event.
    private static let functionKeyCodes: [UInt16: UInt32] = [
        122: 1, 120: 2, 99: 3, 118: 4, 96: 5, 97: 6, 98: 7, 100: 8, 101: 9, 109: 10, 103: 11,
        111: 12,
    ]
    /// Hardware key-code → V2 keypad value. Immutable; not rebuilt per event.
    private static let keypadKeyCodes: [UInt16: UInt32] = [
        82: 0, 83: 1, 84: 2, 85: 3, 86: 4, 87: 5, 88: 6, 89: 7, 91: 8, 92: 9, 65: 10, 75: 11,
        67: 12, 78: 13, 69: 14, 81: 15, 76: 16,
    ]

    static func v2(
        keyCode: UInt16,
        specialKey: NSEvent.SpecialKey?,
        charactersIgnoringModifiers: String?,
        characters: String?,
        modifierFlags: NSEvent.ModifierFlags,
        optionAsAlt: Bool
    ) -> TerminalNativeKeyV2? {
        let flags = modifierFlags.intersection(.deviceIndependentFlagsMask)
        if let function = functionKeyCodes[keyCode] {
            return assembleV2(
                kind: 15, value: function, semantic: true, flags: flags, optionAsAlt: optionAsAlt,
                characters: characters)
        }
        if flags.contains(.numericPad) {
            if let value = keypadKeyCodes[keyCode] {
                return assembleV2(
                    kind: 16, value: value, semantic: true, flags: flags, optionAsAlt: optionAsAlt,
                    characters: characters)
            }
        }
        let kind: UInt16
        let value: UInt32
        var semantic = specialKey != nil
        switch specialKey {
        case .carriageReturn, .newline, .enter:
            kind = 1
            value = 0
        case .tab, .backTab:
            kind = 2
            value = 0
        case .backspace:
            kind = 3
            value = 0
        case .upArrow: kind = 5; value = 0
        case .downArrow: kind = 6; value = 0
        case .rightArrow: kind = 7; value = 0
        case .leftArrow: kind = 8; value = 0
        case .home: kind = 9; value = 0
        case .end: kind = 10; value = 0
        case .insert: kind = 11; value = 0
        case .delete: kind = 12; value = 0
        case .pageUp: kind = 13; value = 0
        case .pageDown: kind = 14; value = 0
        default:
            if charactersIgnoringModifiers == "\u{1b}" {
                kind = 4
                value = 0
                semantic = true
            } else {
                guard let chars = charactersIgnoringModifiers, chars.unicodeScalars.count == 1,
                    let scalar = chars.unicodeScalars.first?.value, (0x20...0x7e).contains(scalar)
                else { return nil }
                var preview: UInt16 = 0
                if flags.contains(.option) && optionAsAlt { preview |= 2 }
                if flags.contains(.control) { preview |= 4 }
                guard preview & 6 != 0 else { return nil }
                kind = 17
                value = scalar >= 0x41 && scalar <= 0x5a ? scalar + 0x20 : scalar
                semantic = false
            }
        }
        return assembleV2(
            kind: kind, value: value, semantic: semantic, flags: flags, optionAsAlt: optionAsAlt,
            characters: characters)
    }

    private static func assembleV2(
        kind: UInt16,
        value: UInt32,
        semantic: Bool,
        flags: NSEvent.ModifierFlags,
        optionAsAlt: Bool,
        characters: String?
    ) -> TerminalNativeKeyV2? {
        var modifiers: UInt16 = 0
        if flags.contains(.shift) { modifiers |= 1 }
        if flags.contains(.option) && (optionAsAlt || semantic) { modifiers |= 2 }
        if flags.contains(.control) { modifiers |= 4 }
        let shiftedASCII: UInt32
        if kind == 17, flags.contains(.shift) {
            guard let chars = characters, chars.unicodeScalars.count == 1,
                let scalar = chars.unicodeScalars.first?.value, (0x20...0x7e).contains(scalar)
            else { return nil }
            shiftedASCII = scalar
        } else {
            shiftedASCII = 0
        }
        return TerminalNativeKeyV2(
            kind: kind, modifiers: modifiers, value: value, shiftedASCII: shiftedASCII)
    }

    static func controlASCII(
        modifierFlags: NSEvent.ModifierFlags,
        charactersIgnoringModifiers: String?
    ) -> UInt32? {
        let flags = modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard flags.contains(.control) else { return nil }
        let allowed: NSEvent.ModifierFlags = [.control, .shift, .capsLock]
        guard flags.subtracting(allowed).isEmpty,
            let candidate = charactersIgnoringModifiers,
            candidate.unicodeScalars.count == 1,
            let scalar = candidate.unicodeScalars.first?.value,
            scalar <= 0x7f
        else {
            return nil
        }
        let normalized: UInt32
        if scalar >= 0x61 && scalar <= 0x7a {
            normalized = scalar - 0x20
        } else {
            normalized = scalar
        }
        return matchesControlBase(normalized) ? normalized : nil
    }

    fileprivate static func semanticKey(
        specialKey: NSEvent.SpecialKey?,
        charactersIgnoringModifiers: String?,
        modifierFlags: NSEvent.ModifierFlags
    ) -> TerminalKeyIntent? {
        let candidate: TerminalKeyIntent?
        switch specialKey {
        case .carriageReturn, .newline, .enter:
            candidate = .enter
        case .tab:
            candidate = .tab
        case .backspace:
            candidate = .backspace
        case .upArrow:
            candidate = .arrowUp
        case .downArrow:
            candidate = .arrowDown
        case .rightArrow:
            candidate = .arrowRight
        case .leftArrow:
            candidate = .arrowLeft
        default:
            switch charactersIgnoringModifiers {
            case "\r", "\n": candidate = .enter
            case "\t": candidate = .tab
            case "\u{8}", "\u{7f}": candidate = .backspace
            case "\u{1b}": candidate = .escape
            default: candidate = nil
            }
        }
        guard let candidate else { return nil }
        let flags = modifierFlags.intersection(.deviceIndependentFlagsMask)
        var allowed: NSEvent.ModifierFlags = [.capsLock]
        if candidate == .enter {
            allowed.insert(.numericPad)
        }
        return flags.subtracting(allowed).isEmpty ? candidate : nil
    }

    private static func matchesControlBase(_ scalar: UInt32) -> Bool {
        scalar == 0x20
            || scalar == 0x3f
            || scalar == 0x40
            || (scalar >= 0x41 && scalar <= 0x5f)
    }
}

/// IME/first-responder adapter. Presentation eligibility is read from Rust.
/// Native keyboard classification forwards TerminalKeyV2; Rust owns encoding.
@MainActor
final class InteractiveMetalSurfaceView: MetalSurfaceView, @preconcurrency NSTextInputClient {
    private let appHandle: UInt64
    private let optionAsAlt: Bool
    private var composition = CompositionDocument()
    private var nextKeyboardActionID: UInt32 = 1
    private var nextMouseActionID: UInt32 = 1
    private var heldKeyboardKinds: [UInt16: TerminalNativeKeyV2] = [:]
    private static let maxHeldKeyboardKinds = 256
    var onBridgeBecameUsable: (() -> Void)?
    var onRequestComposerFocus: (() -> Void)?
    var observedAlternateScreen = false
    private var announcedBridgeUsable = false
    private var mouseTrackingArea: NSTrackingArea?

    init(frame frameRect: NSRect, appHandle: UInt64) {
        self.appHandle = appHandle
        self.optionAsAlt = seyal_app_option_as_alt(appHandle) != 0
        super.init(frame: frameRect, paneID: "m001-pane")
        wantsLayer = true
        setAccessibilityIdentifier("terminal-input")
        setAccessibilityRole(.textArea)
        setAccessibilityElement(true)
    }

    override func restoreNativeInteractionAfterRendererReady() -> Bool {
        if seyal_app_snapshot(appHandle).eligibility == UInt16(SEYAL_APP_ELIGIBILITY_FLOW.rawValue) {
            onRequestComposerFocus?()
            return true
        }
        return window?.makeFirstResponder(self) ?? false
    }

    override func terminalBridgeStatusDidChange() {
        super.terminalBridgeStatusDidChange()
        let connected = terminalBridgeIsConnected
        if connected {
            guard !announcedBridgeUsable else { return }
            announcedBridgeUsable = true
            onBridgeBecameUsable?()
        } else {
            announcedBridgeUsable = false
            heldKeyboardKinds.removeAll(keepingCapacity: true)
            nextKeyboardActionID = 1
            nextMouseActionID = 1
            composition.clear()
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

    override func mouseDown(with event: NSEvent) {
        submitNativeMouse(event, kind: 1)
    }

    override func mouseUp(with event: NSEvent) {
        submitNativeMouse(event, kind: 2)
    }

    override func mouseDragged(with event: NSEvent) {
        submitNativeMouse(event, kind: 3)
    }

    override func mouseMoved(with event: NSEvent) {
        submitNativeMouse(event, kind: 3)
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let mouseTrackingArea {
            removeTrackingArea(mouseTrackingArea)
        }
        let area = NSTrackingArea(
            rect: bounds,
            options: [.mouseMoved, .activeInKeyWindow, .inVisibleRect],
            owner: self,
            userInfo: nil
        )
        addTrackingArea(area)
        mouseTrackingArea = area
    }

    override func rightMouseDown(with event: NSEvent) {
        submitNativeMouse(event, kind: 1)
    }

    override func rightMouseUp(with event: NSEvent) {
        submitNativeMouse(event, kind: 2)
    }

    override func rightMouseDragged(with event: NSEvent) {
        submitNativeMouse(event, kind: 3)
    }

    override func otherMouseDown(with event: NSEvent) {
        submitNativeMouse(event, kind: 1)
    }

    override func otherMouseUp(with event: NSEvent) {
        submitNativeMouse(event, kind: 2)
    }

    override func otherMouseDragged(with event: NSEvent) {
        submitNativeMouse(event, kind: 3)
    }

    override func scrollWheel(with event: NSEvent) {
        guard allowsDirectTerminalInput else {
            super.scrollWheel(with: event)
            return
        }
        let button: UInt8
        if abs(event.scrollingDeltaY) >= abs(event.scrollingDeltaX) {
            if event.scrollingDeltaY == 0 { return }
            button = event.scrollingDeltaY > 0 ? 64 : 65
        } else {
            if event.scrollingDeltaX == 0 { return }
            button = event.scrollingDeltaX > 0 ? 66 : 67
        }
        submitNativeMouse(event, kind: 4, buttonOverride: button)
    }

    override func keyDown(with event: NSEvent) {
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if flags.contains(.command) {
            super.keyDown(with: event)
            return
        }
        guard allowsDirectTerminalInput else { return }

        if composition.hasMarkedText, inputContext?.handleEvent(event) == true {
            return
        }

        if terminalSupportsKeyV2(),
            let key = TerminalNativeKeyClassifier.v2(
                keyCode: event.keyCode,
                specialKey: event.specialKey,
                charactersIgnoringModifiers: event.charactersIgnoringModifiers,
                characters: event.characters,
                modifierFlags: event.modifierFlags,
                optionAsAlt: optionAsAlt
            )
        {
            switch Self.planHeldKeyPress(
                held: heldKeyboardKinds,
                keyCode: event.keyCode,
                isRepeat: event.isARepeat,
                maxHeld: Self.maxHeldKeyboardKinds
            ) {
            case .overflow:
                rejectHeldKeyOverflow()
                return
            case .rejectUntrackedRepeat:
                return
            case .submit:
                break
            }
            guard let actionID = takeNextKeyboardActionID() else { return }
            let admitted =
                terminalSubmitKeyV2(
                    kind: key.kind,
                    modifiers: key.modifiers,
                    value: key.value,
                    event: event.isARepeat ? 2 : 1,
                    shiftedASCII: key.shiftedASCII,
                    actionID: actionID
                ) == 0
            Self.recordHeldKeyIfAdmitted(
                into: &heldKeyboardKinds,
                keyCode: event.keyCode,
                key: key,
                admitted: admitted
            )
            return
        }

        if let controlScalar = TerminalNativeKeyClassifier.controlASCII(
            modifierFlags: event.modifierFlags,
            charactersIgnoringModifiers: event.charactersIgnoringModifiers
        ) {
            _ = terminalSubmitKey(kind: TerminalKeyIntent.controlASCII.rawValue, scalar: controlScalar)
            return
        }
        if flags.contains(.control) {
            super.keyDown(with: event)
            return
        }

        if let key = TerminalNativeKeyClassifier.semanticKey(
            specialKey: event.specialKey,
            charactersIgnoringModifiers: event.charactersIgnoringModifiers,
            modifierFlags: event.modifierFlags
        ) {
            _ = terminalSubmitKey(kind: key.rawValue, scalar: 0)
            return
        }

        if event.specialKey != nil {
            super.keyDown(with: event)
            return
        }

        interpretKeyEvents([event])
    }

    override func keyUp(with event: NSEvent) {
        guard allowsDirectTerminalInput else {
            heldKeyboardKinds.removeValue(forKey: event.keyCode)
            return
        }
        guard let key = Self.takeHeldKeyForV2Release(
            from: &heldKeyboardKinds,
            keyCode: event.keyCode,
            v2Supported: terminalSupportsKeyV2()
        ) else { return }
        guard let actionID = takeNextKeyboardActionID() else { return }
        _ = terminalSubmitKeyV2(
            kind: key.kind,
            modifiers: key.modifiers,
            value: key.value,
            event: 3,
            shiftedASCII: key.shiftedASCII,
            actionID: actionID
        )
    }

    override func viewWillMove(toWindow newWindow: NSWindow?) {
        if newWindow == nil {
            heldKeyboardKinds.removeAll(keepingCapacity: true)
            composition.clear()
        }
        super.viewWillMove(toWindow: newWindow)
    }

    func hasMarkedText() -> Bool { composition.hasMarkedText }

    func markedRange() -> NSRange { composition.markedRange }

    func selectedRange() -> NSRange { composition.selectedRange }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        guard let text = Self.extractPlainString(from: string) else {
            composition.clear()
            return
        }
        do {
            try composition.setMarkedText(
                text, selectedRange: selectedRange, replacementRange: replacementRange)
            inputContext?.invalidateCharacterCoordinates()
        } catch {
            composition.clear()
            scheduleDiscardMarkedText()
        }
    }

    func unmarkText() {
        guard composition.hasMarkedText else { return }
        let text = composition.text
        composition.clear()
        submitIfAllowed(text)
        inputContext?.invalidateCharacterCoordinates()
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?)
        -> NSAttributedString?
    {
        guard let (value, returned) = composition.attributedSubstring(for: range) else {
            actualRange?.pointee = NSRange(location: NSNotFound, length: 0)
            return nil
        }
        actualRange?.pointee = returned
        return value
    }

    func insertText(_ string: Any, replacementRange: NSRange) {
        guard let text = Self.extractPlainString(from: string),
            composition.validatesReplacementRange(replacementRange)
        else {
            composition.clear()
            return
        }
        composition.clear()
        submitIfAllowed(text)
        inputContext?.invalidateCharacterCoordinates()
    }

    func characterIndex(for point: NSPoint) -> Int { NSNotFound }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        let valid: NSRange?
        if composition.hasMarkedText {
            valid = composition.validatedCoordinateRange(range)
        } else if range.location == 0 && range.length == 0 {
            valid = range
        } else {
            valid = nil
        }
        actualRange?.pointee = valid ?? NSRange(location: NSNotFound, length: 0)
        return window?.convertToScreen(convert(bounds, to: nil)) ?? .zero
    }

    override func doCommand(by selector: Selector) {
        _ = selector
    }

    func copy(_ sender: Any?) {
        _ = terminalSubmitHostSelection(action: 4)
    }

    func paste(_ sender: Any?) {
        guard allowsDirectTerminalInput else { return }
        if let text = NSPasteboard.general.string(forType: .string), !text.isEmpty {
            _ = terminalSubmitPaste(text)
        }
    }

    private var allowsDirectTerminalInput: Bool {
        let eligibility = seyal_app_snapshot(appHandle).eligibility
        return eligibility == UInt16(SEYAL_APP_ELIGIBILITY_RAW.rawValue)
            || eligibility == UInt16(SEYAL_APP_ELIGIBILITY_TUI.rawValue)
    }

    private func submitIfAllowed(_ text: String) {
        guard allowsDirectTerminalInput, !text.isEmpty else { return }
        if text.utf8.count > maxCompositionUTF8Bytes { return }
        _ = terminalSubmitCommittedText(text)
    }

    /// SPEC-006 §21.3: held-key overflow rejects the new press under the
    /// existing client-backpressure input-failure wording, VoiceOver-visible.
    private func rejectHeldKeyOverflow() {
        let message = "Input not sent: terminal client is busy. Retry the input."
        setAccessibilityValue(message)
        SeyalAccessibilityAnnouncement.post(message, element: self)
    }

    private func submitNativeMouse(_ event: NSEvent, kind: UInt8, buttonOverride: UInt8? = nil) {
        if !allowsDirectTerminalInput {
            if kind == 1 {
                onRequestComposerFocus?()
            }
            return
        }
        window?.makeFirstResponder(self)
        guard let cell = terminalMouseCell(for: event),
            let actionID = takeNextMouseActionID()
        else { return }
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        var modifiers: UInt16 = 0
        if flags.contains(.shift) { modifiers |= 1 }
        if flags.contains(.option) { modifiers |= 2 }
        if flags.contains(.control) { modifiers |= 4 }
        let button: UInt8
        if let buttonOverride {
            button = buttonOverride
        } else if let mapped = Self.xtermButton(event.buttonNumber) {
            button = mapped
        } else {
            return
        }
        _ = terminalSubmitMouse(
            kind: kind,
            button: button,
            modifiers: modifiers,
            col: cell.0,
            row: cell.1,
            actionID: actionID
        )
    }

    private static func xtermButton(_ buttonNumber: Int) -> UInt8? {
        switch buttonNumber {
        case 0: return 0
        case 1: return 2
        case 2: return 1
        default: return nil
        }
    }

    private static func xtermButtonSelfTest() -> Bool {
        xtermButton(0) == 0
            && xtermButton(1) == 2
            && xtermButton(2) == 1
            && xtermButton(3) == nil
            && xtermButton(-1) == nil
    }

    private func takeNextMouseActionID() -> UInt32? {
        guard let actionID = Self.v2ActionIDBeforeExhaustion(nextMouseActionID) else {
            terminalStopForProtocolRecovery()
            return nil
        }
        nextMouseActionID = actionID + 1
        return actionID
    }

    private func takeNextKeyboardActionID() -> UInt32? {
        guard let actionID = Self.v2ActionIDBeforeExhaustion(nextKeyboardActionID) else {
            terminalStopForProtocolRecovery()
            return nil
        }
        nextKeyboardActionID &+= 1
        return actionID
    }

    private func scheduleDiscardMarkedText() {
        DispatchQueue.main.async { [weak self] in
            self?.inputContext?.discardMarkedText()
        }
    }

    private static func extractPlainString(from value: Any) -> String? {
        if let text = value as? String { return text }
        if let attributed = value as? NSAttributedString { return attributed.string }
        if let value = value as? NSString { return value as String }
        return nil
    }

    /// SPEC-006 §21.5: stop admission before wrapping or replaying action IDs.
    static func v2ActionIDBeforeExhaustion(_ next: UInt32) -> UInt32? {
        (next == 0 || next == .max) ? nil : next
    }

    enum HeldKeyPressPlan: Equatable {
        case overflow
        case rejectUntrackedRepeat
        case submit
    }

    /// New presses occupy a held slot only after V2 admission succeeds.
    /// Repeats of a key that was never admitted are dropped so key-up cannot
    /// synthesize an orphan release.
    static func planHeldKeyPress(
        held: [UInt16: TerminalNativeKeyV2],
        keyCode: UInt16,
        isRepeat: Bool,
        maxHeld: Int
    ) -> HeldKeyPressPlan {
        if held[keyCode] != nil {
            return .submit
        }
        if isRepeat {
            return .rejectUntrackedRepeat
        }
        if held.count >= maxHeld {
            return .overflow
        }
        return .submit
    }

    static func recordHeldKeyIfAdmitted(
        into held: inout [UInt16: TerminalNativeKeyV2],
        keyCode: UInt16,
        key: TerminalNativeKeyV2,
        admitted: Bool
    ) {
        guard admitted, held[keyCode] == nil else { return }
        held[keyCode] = key
    }

    static func takeHeldKeyForV2Release(
        from held: inout [UInt16: TerminalNativeKeyV2],
        keyCode: UInt16,
        v2Supported: Bool
    ) -> TerminalNativeKeyV2? {
        guard v2Supported else {
            held.removeAll(keepingCapacity: true)
            return nil
        }
        return held.removeValue(forKey: keyCode)
    }

    static func pass7InputSelfTest() -> Bool {
        controlNormalizationSelfTest()
            && compositionUTF16SelfTest()
            && compositionBoundsSelfTest()
            && composedSubstringSelfTest()
            && semanticKeyMatrixSelfTest()
            && keyReleaseMetadataSelfTest()
            && heldKeyboardCapacitySelfTest()
            && heldKeyAdmissionSelfTest()
            && capabilityLossDropsHeldKeyReleaseSelfTest()
            && RustDisplayBridge.pasteAdmissionSelfTest()
            && xtermButtonSelfTest()
    }

    private static func controlNormalizationSelfTest() -> Bool {
        let control: NSEvent.ModifierFlags = [.control]
        let shifted: NSEvent.ModifierFlags = [.control, .shift]
        let caps: NSEvent.ModifierFlags = [.control, .capsLock]
        guard
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: control, charactersIgnoringModifiers: "a") == 0x41,
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: caps, charactersIgnoringModifiers: "z") == 0x5a,
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: shifted, charactersIgnoringModifiers: "@") == 0x40,
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: shifted, charactersIgnoringModifiers: "^") == 0x5e,
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: shifted, charactersIgnoringModifiers: "_") == 0x5f,
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: shifted, charactersIgnoringModifiers: "?") == 0x3f,
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: control, charactersIgnoringModifiers: " ") == 0x20,
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: control, charactersIgnoringModifiers: "[") == 0x5b,
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: control, charactersIgnoringModifiers: "\\") == 0x5c,
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: control, charactersIgnoringModifiers: "]") == 0x5d,
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: [.control, .option], charactersIgnoringModifiers: "a") == nil,
            TerminalNativeKeyClassifier.controlASCII(
                modifierFlags: control, charactersIgnoringModifiers: "å") == nil
        else {
            return false
        }
        return TerminalNativeKeyClassifier.controlASCII(
            modifierFlags: control, charactersIgnoringModifiers: "q") == 0x51
    }

    private static func compositionUTF16SelfTest() -> Bool {
        var document = CompositionDocument()
        do {
            try document.setMarkedText(
                "😀x",
                selectedRange: NSRange(location: 2, length: 0),
                replacementRange: NSRange(location: NSNotFound, length: 0)
            )
        } catch {
            return false
        }
        guard document.utf16Length == 3,
            document.markedRange == NSRange(location: 0, length: 3),
            document.selectedRange == NSRange(location: 2, length: 0)
        else {
            return false
        }
        do {
            try document.setMarkedText(
                "A",
                selectedRange: NSRange(location: 1, length: 0),
                replacementRange: NSRange(location: 2, length: 1)
            )
        } catch {
            return false
        }
        return document.text == "😀A" && document.selectedRange == NSRange(location: 3, length: 0)
    }

    private static func compositionBoundsSelfTest() -> Bool {
        var document = CompositionDocument()
        let original = document
        let oversized = String(repeating: "x", count: maxCompositionUTF8Bytes + 1)
        do {
            try document.setMarkedText(
                oversized,
                selectedRange: NSRange(location: 0, length: 0),
                replacementRange: NSRange(location: NSNotFound, length: 0)
            )
            return false
        } catch CompositionMutationError.tooLarge {
            guard document == original else { return false }
        } catch {
            return false
        }
        do {
            try document.setMarkedText(
                "x",
                selectedRange: NSRange(location: 2, length: 0),
                replacementRange: NSRange(location: NSNotFound, length: 0)
            )
            return false
        } catch CompositionMutationError.invalidRange {
            return document == original
        } catch {
            return false
        }
    }

    private static func composedSubstringSelfTest() -> Bool {
        var document = CompositionDocument()
        do {
            try document.setMarkedText(
                "e\u{301}",
                selectedRange: NSRange(location: 2, length: 0),
                replacementRange: NSRange(location: NSNotFound, length: 0)
            )
        } catch {
            return false
        }
        guard
            let (substring, range) = document.attributedSubstring(for: NSRange(location: 1, length: 1))
        else {
            return false
        }
        return substring.string == "e\u{301}"
            && range == NSRange(location: 0, length: 2)
            && document.attributedSubstring(for: NSRange(location: 3, length: 1)) == nil
    }

    private static func semanticKeyMatrixSelfTest() -> Bool {
        TerminalNativeKeyClassifier.semanticKey(
            specialKey: .upArrow, charactersIgnoringModifiers: nil, modifierFlags: []) == .arrowUp
            && TerminalNativeKeyClassifier.semanticKey(
                specialKey: .upArrow, charactersIgnoringModifiers: nil, modifierFlags: [.shift])
                == nil
            && TerminalNativeKeyClassifier.semanticKey(
                specialKey: .enter, charactersIgnoringModifiers: nil, modifierFlags: [.numericPad])
                == .enter
            && TerminalNativeKeyClassifier.semanticKey(
                specialKey: .backTab, charactersIgnoringModifiers: nil, modifierFlags: [.shift])
                == nil
            && TerminalNativeKeyClassifier.semanticKey(
                specialKey: .delete, charactersIgnoringModifiers: nil, modifierFlags: []) == nil
            && TerminalNativeKeyClassifier.v2(
                keyCode: 36, specialKey: .enter, charactersIgnoringModifiers: nil, characters: nil,
                modifierFlags: [], optionAsAlt: false)?.kind == 1
            && TerminalNativeKeyClassifier.v2(
                keyCode: 48, specialKey: .tab, charactersIgnoringModifiers: nil, characters: nil,
                modifierFlags: [.shift], optionAsAlt: false
            ).map { $0.kind == 2 && $0.modifiers == 1 } == true
            && TerminalNativeKeyClassifier.v2(
                keyCode: 53, specialKey: nil, charactersIgnoringModifiers: "\u{1b}",
                characters: nil, modifierFlags: [], optionAsAlt: false)?.kind == 4
            && TerminalNativeKeyClassifier.v2(
                keyCode: 36, specialKey: .enter, charactersIgnoringModifiers: nil, characters: nil,
                modifierFlags: [.control], optionAsAlt: false
            ).map { $0.kind == 1 && $0.modifiers == 4 } == true
            && TerminalNativeKeyClassifier.v2(
                keyCode: 51, specialKey: .backspace, charactersIgnoringModifiers: nil,
                characters: nil, modifierFlags: [.control], optionAsAlt: false
            ).map { $0.kind == 3 && $0.modifiers == 4 } == true
            && InteractiveMetalSurfaceView.v2ActionIDBeforeExhaustion(1) == 1
            && InteractiveMetalSurfaceView.v2ActionIDBeforeExhaustion(0) == nil
            && InteractiveMetalSurfaceView.v2ActionIDBeforeExhaustion(.max) == nil
            && TerminalNativeKeyClassifier.v2(
                keyCode: 0, specialKey: nil, charactersIgnoringModifiers: "a", characters: "a",
                modifierFlags: [], optionAsAlt: false) == nil
            && TerminalNativeKeyClassifier.v2(
                keyCode: 0, specialKey: nil, charactersIgnoringModifiers: "a", characters: "A",
                modifierFlags: [.shift, .option], optionAsAlt: true)?.shiftedASCII == 65
    }

    private static func heldKeyboardCapacitySelfTest() -> Bool {
        var held: [UInt16: TerminalNativeKeyV2] = [:]
        for index in 0..<maxHeldKeyboardKinds {
            held[UInt16(index)] = TerminalNativeKeyV2(
                kind: 17, modifiers: 0, value: 0x61, shiftedASCII: 0)
        }
        let trackedRepeatAllowed =
            planHeldKeyPress(
                held: held, keyCode: 0, isRepeat: true, maxHeld: maxHeldKeyboardKinds) == .submit
        let newPressRejected =
            planHeldKeyPress(
                held: held, keyCode: 300, isRepeat: false, maxHeld: maxHeldKeyboardKinds)
            == .overflow
        return trackedRepeatAllowed && newPressRejected
    }

    private static func heldKeyAdmissionSelfTest() -> Bool {
        let key = TerminalNativeKeyV2(kind: 5, modifiers: 0, value: 0, shiftedASCII: 0)
        var held: [UInt16: TerminalNativeKeyV2] = [:]
        guard
            planHeldKeyPress(held: held, keyCode: 1, isRepeat: false, maxHeld: 2) == .submit
        else { return false }
        recordHeldKeyIfAdmitted(into: &held, keyCode: 1, key: key, admitted: false)
        guard held.isEmpty else { return false }
        guard
            planHeldKeyPress(held: held, keyCode: 1, isRepeat: true, maxHeld: 2)
                == .rejectUntrackedRepeat
        else { return false }
        recordHeldKeyIfAdmitted(into: &held, keyCode: 1, key: key, admitted: true)
        guard held[1] != nil else { return false }
        guard planHeldKeyPress(held: held, keyCode: 1, isRepeat: true, maxHeld: 2) == .submit
        else { return false }
        recordHeldKeyIfAdmitted(into: &held, keyCode: 1, key: key, admitted: true)
        recordHeldKeyIfAdmitted(into: &held, keyCode: 2, key: key, admitted: true)
        return planHeldKeyPress(held: held, keyCode: 3, isRepeat: false, maxHeld: 2) == .overflow
    }

    private static func keyReleaseMetadataSelfTest() -> Bool {
        guard
            let key = TerminalNativeKeyClassifier.v2(
                keyCode: 0, specialKey: nil, charactersIgnoringModifiers: "a", characters: "A",
                modifierFlags: [.shift, .option], optionAsAlt: true)
        else { return false }
        var held: [UInt16: TerminalNativeKeyV2] = [0: key]
        let released = held.removeValue(forKey: 0)
        return released?.shiftedASCII == 65 && held.isEmpty
    }

    private static func capabilityLossDropsHeldKeyReleaseSelfTest() -> Bool {
        guard
            let key = TerminalNativeKeyClassifier.v2(
                keyCode: 0, specialKey: nil, charactersIgnoringModifiers: "a", characters: "A",
                modifierFlags: [.shift, .option], optionAsAlt: true)
        else { return false }
        var held: [UInt16: TerminalNativeKeyV2] = [0: key]
        let released = takeHeldKeyForV2Release(from: &held, keyCode: 0, v2Supported: false)
        return released == nil && held.isEmpty
    }
}
