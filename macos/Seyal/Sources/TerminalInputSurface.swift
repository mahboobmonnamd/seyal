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

private enum NativeInputFailure: Int32 {
  case clientBackpressure = 1
  case commitTooLarge = 2
  case lostController = 3
  case disconnected = 4
  case unsupportedReplacementRange = 5
  case compositionTooLarge = 6

  var message: String {
    switch self {
    case .clientBackpressure:
      "Input not sent: terminal client is busy. Retry the input."
    case .commitTooLarge:
      "Input not sent: committed text exceeds the 64 KiB M001 limit."
    case .lostController:
      "Input unavailable: this surface does not hold terminal control."
    case .disconnected:
      "Input unavailable: the terminal Runtime connection is unavailable."
    case .unsupportedReplacementRange:
      "Input not sent: the input method requested an unsupported replacement range."
    case .compositionTooLarge:
      "Input method composition exceeded the 64 KiB M001 limit and was cancelled."
    }
  }
}

private enum CompositionMutationError: Error, Equatable {
  case invalidRange
  case tooLarge
}

private struct CompositionDocument: Equatable {
  private(set) var text = ""
  private(set) var selection = NSRange(location: 0, length: 0)

  var utf16Length: Int {
    (text as NSString).length
  }

  var hasMarkedText: Bool {
    utf16Length > 0
  }

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
    let absoluteSelection = NSRange(
      location: absoluteLocation,
      length: insertedSelection.length
    )
    guard
      Self.validatedRange(
        absoluteSelection,
        upperBound: (candidate as NSString).length
      ) != nil
    else {
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
    let bounded = NSRange(
      location: boundedStart,
      length: boundedEnd - boundedStart
    )
    if bounded.length == 0 {
      return (NSAttributedString(string: ""), bounded)
    }

    let storage = text as NSString
    let composed = storage.rangeOfComposedCharacterSequences(for: bounded)
    guard let valid = Self.validatedRange(composed, upperBound: length) else { return nil }
    return (
      NSAttributedString(string: storage.substring(with: valid)),
      valid
    )
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
    let bounded = NSRange(
      location: range.location,
      length: min(end, length) - range.location
    )
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

private struct TerminalLayoutSample: Equatable {
  let viewportWidth: Double
  let viewportHeight: Double
  let horizontalInsets: Double
  let verticalInsets: Double
  let cellWidth: Double
  let cellHeight: Double
}

private enum TerminalNativeKeyClassifier {
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

  static func semanticKey(
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

@MainActor
final class InteractiveMetalSurfaceView: MetalSurfaceView, @MainActor NSTextInputClient {
  private var composition = CompositionDocument()
  private var lastLayoutSample: TerminalLayoutSample?
  private var nativeFailure: NativeInputFailure?
  private let failureLayer = CATextLayer()

  convenience init(frame frameRect: NSRect) {
    self.init(frame: frameRect, paneID: "unbound")
  }

  override init(
    frame frameRect: NSRect,
    paneID: String,
    executionIdentity: String? = nil,
    allowsImplicitExecutionBootstrap: Bool = true,
    terminalFont: SeyalResolvedFontSpec = .canonicalTerminal
  ) {
    super.init(
      frame: frameRect,
      paneID: paneID,
      executionIdentity: executionIdentity,
      allowsImplicitExecutionBootstrap: allowsImplicitExecutionBootstrap,
      terminalFont: terminalFont
    )
    configureInteractiveSurface()
  }

  private func configureInteractiveSurface() {
    configureAccessibility()
    refreshRecoveryAccessibilityValue()
    configureFailureLayer()
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) {
    fatalError("Seyal uses a programmatic AppKit/Metal surface")
  }

  override var acceptsFirstResponder: Bool {
    true
  }

  override func becomeFirstResponder() -> Bool {
    guard super.becomeFirstResponder() else { return false }
    inputContext?.activate()
    setAccessibilityFocused(true)
    refreshFailurePresentation()
    return true
  }

  override func resignFirstResponder() -> Bool {
    cancelComposition(discardInputContext: true)
    inputContext?.deactivate()
    setAccessibilityFocused(false)
    return super.resignFirstResponder()
  }

  /// SPEC-009 §10 native interaction restore on the production interactive surface.
  @discardableResult
  override func restoreNativeInteractionAfterRendererReady() -> Bool {
    guard let window else { return false }
    if !window.isVisible {
      window.orderFront(nil)
    }
    if !window.isKeyWindow {
      window.makeKey()
    }
    guard window.makeFirstResponder(self) else { return false }
    guard isAccessibilityFocused() else { return false }
    guard !hasMarkedText() else { return false }
    if let inputContext {
      inputContext.activate()
    }
    if !suppressesAutomaticBridgeRecovery {
      refreshRecoveryAccessibilityValue()
    }
    return true
  }

  override func mouseDown(with event: NSEvent) {
    window?.makeFirstResponder(self)
    super.mouseDown(with: event)
  }

  override func keyDown(with event: NSEvent) {
    let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
    if flags.contains(.command) {
      super.keyDown(with: event)
      return
    }

    if composition.hasMarkedText,
      inputContext?.handleEvent(event) == true
    {
      return
    }

    if let controlScalar = TerminalNativeKeyClassifier.controlASCII(
      modifierFlags: event.modifierFlags,
      charactersIgnoringModifiers: event.charactersIgnoringModifiers
    ) {
      submitSemanticKey(.controlASCII, scalar: controlScalar)
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
      submitSemanticKey(key, scalar: 0)
      return
    }

    if event.specialKey != nil {
      super.keyDown(with: event)
      return
    }

    interpretKeyEvents([event])
  }

  override func layout() {
    super.layout()
    layoutFailureLayer()
    synchronizeTerminalGeometry()
    inputContext?.invalidateCharacterCoordinates()
  }

  override func viewDidChangeBackingProperties() {
    super.viewDidChangeBackingProperties()
    synchronizeTerminalGeometry()
    inputContext?.invalidateCharacterCoordinates()
  }

  override func viewWillMove(toWindow newWindow: NSWindow?) {
    if newWindow == nil {
      cancelComposition(discardInputContext: true)
    }
    super.viewWillMove(toWindow: newWindow)
  }

  override func terminalBridgeDidFail(_ code: Int32) {
    super.terminalBridgeDidFail(code)
    cancelComposition(discardInputContext: true)
    nativeFailure = .disconnected
    refreshFailurePresentation()
  }

  override func terminalBridgeStatusDidChange() {
    super.terminalBridgeStatusDidChange()
    if terminalBridgeIsConnected, nativeFailure == .disconnected {
      nativeFailure = nil
    }
    refreshFailurePresentation()
  }

  func hasMarkedText() -> Bool {
    composition.hasMarkedText
  }

  func markedRange() -> NSRange {
    composition.markedRange
  }

  func selectedRange() -> NSRange {
    composition.selectedRange
  }

  func setMarkedText(
    _ string: Any,
    selectedRange: NSRange,
    replacementRange: NSRange
  ) {
    guard let text = Self.plainString(from: string) else {
      failComposition(.unsupportedReplacementRange)
      return
    }
    do {
      try composition.setMarkedText(
        text,
        selectedRange: selectedRange,
        replacementRange: replacementRange
      )
      nativeFailure = nil
      inputContext?.invalidateCharacterCoordinates()
      refreshFailurePresentation()
    } catch CompositionMutationError.tooLarge {
      failComposition(.compositionTooLarge)
    } catch {
      failComposition(.unsupportedReplacementRange)
    }
  }

  func unmarkText() {
    guard composition.hasMarkedText else { return }
    let text = composition.text
    composition.clear()
    _ = submitCommittedText(text)
    inputContext?.invalidateCharacterCoordinates()
  }

  func validAttributesForMarkedText() -> [NSAttributedString.Key] {
    []
  }

  func attributedSubstring(
    forProposedRange range: NSRange,
    actualRange: NSRangePointer?
  ) -> NSAttributedString? {
    guard let (substring, returnedRange) = composition.attributedSubstring(for: range) else {
      actualRange?.pointee = NSRange(location: NSNotFound, length: 0)
      return nil
    }
    actualRange?.pointee = returnedRange
    return substring
  }

  func insertText(_ string: Any, replacementRange: NSRange) {
    guard let text = Self.plainString(from: string),
      composition.validatesReplacementRange(replacementRange)
    else {
      failComposition(.unsupportedReplacementRange)
      return
    }

    composition.clear()
    _ = submitCommittedText(text)
    inputContext?.invalidateCharacterCoordinates()
  }

  func firstRect(
    forCharacterRange range: NSRange,
    actualRange: NSRangePointer?
  ) -> NSRect {
    let valid: NSRange?
    if composition.hasMarkedText {
      valid = composition.validatedCoordinateRange(range)
    } else if range.location == 0 && range.length == 0 {
      valid = range
    } else {
      valid = nil
    }
    actualRange?.pointee = valid ?? NSRange(location: NSNotFound, length: 0)
    return terminalCandidateAnchorRectInScreenCoordinates()
  }

  func characterIndex(for point: NSPoint) -> Int {
    _ = point
    return NSNotFound
  }

  override func doCommand(by selector: Selector) {
    // The event has reached the input-system command seam. Pass 7 never
    // invokes arbitrary editing selectors against terminal/history state.
    _ = selector
  }

  private func synchronizeTerminalGeometry() {
    guard terminalBridgeIsConnected else { return }
    let cell = terminalLogicalCellSize()
    let sample = TerminalLayoutSample(
      viewportWidth: Double(bounds.width),
      viewportHeight: Double(bounds.height),
      horizontalInsets: 0,
      verticalInsets: 0,
      cellWidth: Double(cell.width),
      cellHeight: Double(cell.height)
    )
    let meaningfulEpoch = lastLayoutSample != sample
    lastLayoutSample = sample
    let result = terminalProposeGeometry(
      viewportWidth: sample.viewportWidth,
      viewportHeight: sample.viewportHeight,
      horizontalInsets: sample.horizontalInsets,
      verticalInsets: sample.verticalInsets,
      cellWidth: sample.cellWidth,
      cellHeight: sample.cellHeight,
      meaningfulLayoutEpoch: meaningfulEpoch
    )
    // -17 means this transient layout sample is not geometrically valid;
    // SPEC-006 requires no desired mutation in that case, not an error loop.
    if result != 0 && result != -17 {
      refreshFailurePresentation()
    }
  }

  @discardableResult
  private func submitCommittedText(_ text: String) -> Int32 {
    if text.utf8.count > maxCompositionUTF8Bytes {
      nativeFailure = .commitTooLarge
      refreshFailurePresentation()
      return -14
    }
    let result = terminalSubmitCommittedText(text)
    if result == 0 {
      nativeFailure = nil
    }
    refreshFailurePresentation()
    return result
  }

  private func submitSemanticKey(_ key: TerminalKeyIntent, scalar: UInt32) {
    let result = terminalSubmitKey(kind: key.rawValue, scalar: scalar)
    if result == 0 {
      nativeFailure = nil
    }
    refreshFailurePresentation()
  }

  private func failComposition(_ failure: NativeInputFailure) {
    // Clear first so there is no hidden replay source. Tell the active input
    // context to abandon conversion only after the current NSTextInputClient
    // callback unwinds; this avoids re-entering the same composition method.
    composition.clear()
    nativeFailure = failure
    inputContext?.invalidateCharacterCoordinates()
    scheduleDiscardMarkedText()
    refreshFailurePresentation()
  }

  private func scheduleDiscardMarkedText() {
    DispatchQueue.main.async { [weak self] in
      self?.inputContext?.discardMarkedText()
    }
  }

  private func cancelComposition(discardInputContext: Bool) {
    if discardInputContext {
      inputContext?.discardMarkedText()
    }
    composition.clear()
    inputContext?.invalidateCharacterCoordinates()
  }

  private func terminalCandidateAnchorRectInScreenCoordinates() -> NSRect {
    let cell = terminalPresentationCellSize()
    let finiteHeight = cell.height.isFinite && cell.height > 0 ? cell.height : 1
    var local = NSRect(x: bounds.minX, y: bounds.minY, width: 0, height: finiteHeight)
    if let frame = terminalCurrentFrame(),
      frame.rows > 0,
      frame.columns > 0,
      cell.width.isFinite,
      cell.width > 0,
      cell.height.isFinite,
      cell.height > 0
    {
      let row = min(Int(frame.cursor_row), Int(frame.rows) - 1)
      let column = min(Int(frame.cursor_column), Int(frame.columns) - 1)
      let x = bounds.minX + CGFloat(column) * cell.width
      let y = bounds.maxY - CGFloat(row + 1) * cell.height
      if x.isFinite, y.isFinite {
        local = NSRect(x: x, y: y, width: 0, height: cell.height)
      }
    }
    guard let window else { return .zero }
    return window.convertToScreen(convert(local, to: nil))
  }

  private func configureAccessibility() {
    setAccessibilityElement(true)
    setAccessibilityEnabled(true)
    setAccessibilityRole(.group)
    setAccessibilityRoleDescription("Terminal")
    setAccessibilityLabel("Seyal Terminal")
    setAccessibilityFocused(false)
  }

  private func configureFailureLayer() {
    failureLayer.alignmentMode = .left
    failureLayer.fontSize = 12
    failureLayer.foregroundColor = NSColor.labelColor.cgColor
    failureLayer.backgroundColor =
      NSColor.windowBackgroundColor
      .withAlphaComponent(0.92)
      .cgColor
    failureLayer.cornerRadius = 4
    failureLayer.contentsScale = NSScreen.main?.backingScaleFactor ?? 1
    failureLayer.isHidden = true
    layer?.addSublayer(failureLayer)
    layoutFailureLayer()
  }

  private func layoutFailureLayer() {
    failureLayer.contentsScale =
      window?.backingScaleFactor
      ?? NSScreen.main?.backingScaleFactor
      ?? 1
    failureLayer.frame = CGRect(
      x: 8,
      y: 8,
      width: max(bounds.width - 16, 0),
      height: 22
    )
  }

  private func refreshFailurePresentation() {
    let message = currentFailureMessage()
    failureLayer.string = message
    failureLayer.isHidden = message == nil
    setAccessibilityHelp(message)
  }

  private func currentFailureMessage() -> String? {
    if let nativeFailure {
      return nativeFailure.message
    }
    switch terminalInputFailureCode() {
    case 1: return NativeInputFailure.clientBackpressure.message
    case 2: return NativeInputFailure.commitTooLarge.message
    case 3: return NativeInputFailure.lostController.message
    case 4: return NativeInputFailure.disconnected.message
    default: break
    }

    let resize = terminalResizeFailureCode()
    switch resize {
    case 1:
      return "Terminal resize is waiting for client queue capacity."
    case 200:
      return "Terminal resize stopped because of a protocol inconsistency. Reconnect is required."
    case 201:
      return "Terminal resize is unavailable while the Runtime connection is disconnected."
    case 101...199:
      return "Terminal resize was rejected by the Runtime. Adjust the layout or retry explicitly."
    default:
      return nil
    }
  }

  private static func plainString(from value: Any) -> String? {
    if let value = value as? NSAttributedString {
      return value.string
    }
    if let value = value as? NSString {
      return value as String
    }
    return nil
  }

  static func pass7InputSelfTest() -> Bool {
    controlNormalizationSelfTest()
      && compositionUTF16SelfTest()
      && compositionBoundsSelfTest()
      && composedSubstringSelfTest()
      && semanticKeyMatrixSelfTest()
  }

  private static func controlNormalizationSelfTest() -> Bool {
    let control: NSEvent.ModifierFlags = [.control]
    let shifted: NSEvent.ModifierFlags = [.control, .shift]
    let caps: NSEvent.ModifierFlags = [.control, .capsLock]
    guard
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: control,
        charactersIgnoringModifiers: "a"
      ) == 0x41,
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: caps,
        charactersIgnoringModifiers: "z"
      ) == 0x5a,
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: shifted,
        charactersIgnoringModifiers: "@"
      ) == 0x40,
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: shifted,
        charactersIgnoringModifiers: "^"
      ) == 0x5e,
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: shifted,
        charactersIgnoringModifiers: "_"
      ) == 0x5f,
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: shifted,
        charactersIgnoringModifiers: "?"
      ) == 0x3f,
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: control,
        charactersIgnoringModifiers: " "
      ) == 0x20,
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: control,
        charactersIgnoringModifiers: "["
      ) == 0x5b,
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: control,
        charactersIgnoringModifiers: "\\"
      ) == 0x5c,
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: control,
        charactersIgnoringModifiers: "]"
      ) == 0x5d,
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: [.control, .option],
        charactersIgnoringModifiers: "a"
      ) == nil,
      TerminalNativeKeyClassifier.controlASCII(
        modifierFlags: control,
        charactersIgnoringModifiers: "å"
      ) == nil
    else {
      return false
    }

    // Synthetic layout-derived Q proves classification follows AppKit's
    // scalar and never consults a US physical-key table.
    return TerminalNativeKeyClassifier.controlASCII(
      modifierFlags: control,
      charactersIgnoringModifiers: "q"
    ) == 0x51
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
    return document.text == "😀A"
      && document.selectedRange == NSRange(location: 3, length: 0)
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
      let (substring, range) = document.attributedSubstring(
        for: NSRange(location: 1, length: 1)
      )
    else {
      return false
    }
    return substring.string == "e\u{301}"
      && range == NSRange(location: 0, length: 2)
      && document.attributedSubstring(for: NSRange(location: 3, length: 1)) == nil
  }

  private static func semanticKeyMatrixSelfTest() -> Bool {
    TerminalNativeKeyClassifier.semanticKey(
      specialKey: .upArrow,
      charactersIgnoringModifiers: nil,
      modifierFlags: []
    ) == .arrowUp
      && TerminalNativeKeyClassifier.semanticKey(
        specialKey: .upArrow,
        charactersIgnoringModifiers: nil,
        modifierFlags: [.shift]
      ) == nil
      && TerminalNativeKeyClassifier.semanticKey(
        specialKey: .enter,
        charactersIgnoringModifiers: nil,
        modifierFlags: [.numericPad]
      ) == .enter
      && TerminalNativeKeyClassifier.semanticKey(
        specialKey: .backTab,
        charactersIgnoringModifiers: nil,
        modifierFlags: [.shift]
      ) == nil
      && TerminalNativeKeyClassifier.semanticKey(
        specialKey: .delete,
        charactersIgnoringModifiers: nil,
        modifierFlags: []
      ) == nil
  }
}
