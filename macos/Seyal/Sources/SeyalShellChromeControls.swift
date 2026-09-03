import AppKit

/// Segmented control for left-panel mode with an explicit radio-group accessibility role.
final class SeyalPreviewModeControl: NSSegmentedControl {
  override func accessibilityRole() -> NSAccessibility.Role? { .radioGroup }
  override func isAccessibilityElement() -> Bool { true }
}

/// Selection control with `/apple-design`-style press semantics:
/// preview emphasis on pointer-down, cancel if drag exceeds threshold, commit on pointer-up.
final class SeyalPressableSelectionButton: NSButton {
  var activationDistance: CGFloat = 10

  var onPress: (() -> Void)?
  var onCancel: (() -> Void)?
  var onCommit: (() -> Void)?

  private var pressOrigin: NSPoint?
  private var isCancelled = false

  override func mouseDown(with event: NSEvent) {
    pressOrigin = convert(event.locationInWindow, from: nil)
    isCancelled = false
    window?.makeFirstResponder(self)
    onPress?()
  }

  override func mouseDragged(with event: NSEvent) {
    guard let origin = pressOrigin else { return }
    let current = convert(event.locationInWindow, from: nil)
    let dx = current.x - origin.x
    let dy = current.y - origin.y
    if hypot(dx, dy) > activationDistance, !isCancelled {
      isCancelled = true
      onCancel?()
    }
  }

  override func mouseUp(with event: NSEvent) {
    if isCancelled {
      onCancel?()
      return
    }
    onCommit?()
  }
}
