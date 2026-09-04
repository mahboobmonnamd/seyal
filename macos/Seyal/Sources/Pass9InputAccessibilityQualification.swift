import AppKit
import Foundation

/// Pass 9 release-qualification Track C (partial for #736): dead-key / IME
/// through the production `NSTextInputClient`, plus VoiceOver-*facing* AX field
/// checks. This does **not** enable system VoiceOver or prove focus/announcement
/// after reconnect — that remains an open #736 gate.
@MainActor
enum Pass9InputAccessibilityQualification {
  struct Result: Codable {
    let schema: String
    let commit: String
    let deadKeyMarkedThenCommit: Bool
    let imeCancelWithoutTranscript: Bool
    let imeReplacementCommit: Bool
    let markedTextAbsentFromAccessibilityValue: Bool
    let candidateRectFinite: Bool
    let voiceOverFacingAxFieldsDisconnected: Bool
    let voiceOverFacingAxFieldsAfterRefresh: Bool
    let acceptsFirstResponder: Bool
    let pass7InputSelfTest: Bool
    let overallPass: Bool
    let voiceOverClaim: String
  }

  static func run(commit: String? = nil) -> Bool {
    let resolvedCommit = commit
      ?? ProcessInfo.processInfo.environment["SEYAL_PASS9_EXPECTED_HEAD"]
      ?? gitHead()
      ?? String(repeating: "0", count: 40)
    let checks = executeChecks()
    let overall = checks.values.allSatisfy { $0 }
    let artifact = Result(
      schema: "seyal.pass9.input-accessibility.v1",
      commit: resolvedCommit,
      deadKeyMarkedThenCommit: checks["dead_key_marked_then_commit"] ?? false,
      imeCancelWithoutTranscript: checks["ime_cancel_without_transcript"] ?? false,
      imeReplacementCommit: checks["ime_replacement_commit"] ?? false,
      markedTextAbsentFromAccessibilityValue: checks["marked_text_absent_from_ax_value"] ?? false,
      candidateRectFinite: checks["candidate_rect_finite"] ?? false,
      voiceOverFacingAxFieldsDisconnected: checks["vo_facing_ax_disconnected"] ?? false,
      voiceOverFacingAxFieldsAfterRefresh: checks["vo_facing_ax_after_refresh"] ?? false,
      acceptsFirstResponder: checks["accepts_first_responder"] ?? false,
      pass7InputSelfTest: checks["pass7_input_self_test"] ?? false,
      overallPass: overall,
      voiceOverClaim:
        "VoiceOver-facing AX role/label/frame/recovery fields only; "
        + "system VoiceOver focus/announcement/reconnect discoverability not claimed"
    )

    if let out = outputPathArgument() {
      let encoder = JSONEncoder()
      encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
      do {
        let data = try encoder.encode(artifact)
        try data.write(to: URL(fileURLWithPath: out), options: .atomic)
        print("pass9_input_accessibility_artifact=\(out)")
      } catch {
        print("pass9_input_accessibility_error=write_failed:\(error)")
        return false
      }
    }

    print(
      "pass9_input_accessibility result=\(overall ? "ok" : "fail") "
        + "schema=seyal.pass9.input-accessibility.v1 commit=\(resolvedCommit.prefix(12))"
    )
    for (key, value) in checks.sorted(by: { $0.key < $1.key }) {
      print("pass9_input_accessibility check=\(key) pass=\(value)")
    }
    return overall
  }

  static func executeChecks() -> [String: Bool] {
    var checks = [String: Bool]()
    let surface = InteractiveMetalSurfaceView(
      frame: NSRect(x: 0, y: 0, width: 960, height: 600),
      paneID: "pass9-input-accessibility",
      allowsImplicitExecutionBootstrap: false
    )
    // Attach to a real NSWindow so firstRect screen conversion is non-zero and
    // accessibility geometry is meaningful (detached views return .zero).
    let window = NSWindow(
      contentRect: NSRect(x: 80, y: 80, width: 960, height: 600),
      styleMask: [.titled, .closable],
      backing: .buffered,
      defer: false
    )
    window.contentView = surface
    window.orderFront(nil)
    defer { window.orderOut(nil) }
    surface.layoutSubtreeIfNeeded()
    _ = window.makeFirstResponder(surface)

    checks["accepts_first_responder"] = surface.acceptsFirstResponder
    checks["pass7_input_self_test"] = InteractiveMetalSurfaceView.pass7InputSelfTest()

    // Dead-key style composition: marked base, then IME commit of composed form.
    // Marked text must never reach submit until insertText/unmarkText.
    surface.setMarkedText(
      "e",
      selectedRange: NSRange(location: 1, length: 0),
      replacementRange: NSRange(location: NSNotFound, length: 0)
    )
    let markedAfterDeadKey = surface.hasMarkedText()
    let axDuringMarked = String(describing: surface.accessibilityValue() ?? "")
    let markedLeaked = axDuringMarked.contains("marked=") || axDuringMarked.contains("e\u{301}")
    checks["marked_text_absent_from_ax_value"] = markedAfterDeadKey && !markedLeaked
    surface.insertText("é", replacementRange: NSRange(location: NSNotFound, length: 0))
    checks["dead_key_marked_then_commit"] = markedAfterDeadKey && !surface.hasMarkedText()

    // IME cancel / abandon: clear composition without leaving marked residue.
    surface.setMarkedText(
      "変換中",
      selectedRange: NSRange(location: 3, length: 0),
      replacementRange: NSRange(location: NSNotFound, length: 0)
    )
    let markedBeforeCancel = surface.hasMarkedText()
    _ = surface.resignFirstResponder()
    checks["ime_cancel_without_transcript"] = markedBeforeCancel && !surface.hasMarkedText()
    _ = window.makeFirstResponder(surface)

    // Replacement commit (IME confirms a candidate replacing the marked range).
    surface.setMarkedText(
      "ni",
      selectedRange: NSRange(location: 2, length: 0),
      replacementRange: NSRange(location: NSNotFound, length: 0)
    )
    let markedBeforeReplace = surface.hasMarkedText()
    surface.insertText("に", replacementRange: NSRange(location: 0, length: 2))
    checks["ime_replacement_commit"] = markedBeforeReplace && !surface.hasMarkedText()

    var actual = NSRange(location: 0, length: 0)
    surface.setMarkedText(
      "a",
      selectedRange: NSRange(location: 1, length: 0),
      replacementRange: NSRange(location: NSNotFound, length: 0)
    )
    let rect = surface.firstRect(
      forCharacterRange: NSRange(location: 0, length: 1),
      actualRange: &actual
    )
    checks["candidate_rect_finite"] =
      surface.hasMarkedText()
      && rect.origin.x.isFinite
      && rect.origin.y.isFinite
      && rect.size.width.isFinite
      && rect.size.height.isFinite
      && rect != .zero
      && rect.height > 0
    _ = surface.resignFirstResponder()
    guard !surface.hasMarkedText() else {
      checks["candidate_rect_finite"] = false
      return checks
    }
    _ = window.makeFirstResponder(surface)

    // VoiceOver-facing discovery without enabling system VoiceOver audio:
    // role/label/element flags + recovery value after refresh.
    surface.refreshRecoveryAccessibilityValue()
    let roleOK = surface.accessibilityRole() == .group
    let labelOK = surface.accessibilityLabel() == "Seyal Terminal"
    let elementOK = surface.isAccessibilityElement()
    let frame = surface.accessibilityFrame()
    let value = String(describing: surface.accessibilityValue() ?? "")
    checks["vo_facing_ax_disconnected"] =
      roleOK
      && labelOK
      && elementOK
      && value.contains("connection=disconnected")
      && frame.width > 0
      && frame.height > 0
    surface.refreshRecoveryAccessibilityValue()
    let value2 = String(describing: surface.accessibilityValue() ?? "")
    // Harness surface is intentionally disconnected; require stable typed fields
    // and that refresh does not invent a usable connection claim.
    checks["vo_facing_ax_after_refresh"] =
      value2.contains("connection=disconnected")
      && value2.contains("runtime=none")
      && value2.contains("execution=none")
      && value2.contains("attachment=none")
      && !value2.contains("marked=")
      && surface.accessibilityFrame().width > 0

    return checks
  }

  private static func outputPathArgument() -> String? {
    let args = CommandLine.arguments
    guard let index = args.firstIndex(of: "--output"),
      args.index(after: index) < args.endIndex
    else { return nil }
    return args[args.index(after: index)]
  }

  private static func gitHead() -> String? {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
    process.arguments = ["rev-parse", "HEAD"]
    process.currentDirectoryURL = URL(
      fileURLWithPath: FileManager.default.currentDirectoryPath
    )
    let pipe = Pipe()
    process.standardOutput = pipe
    process.standardError = Pipe()
    do {
      try process.run()
      process.waitUntilExit()
      guard process.terminationStatus == 0 else { return nil }
      let data = pipe.fileHandleForReading.readDataToEndOfFile()
      return String(data: data, encoding: .utf8)?
        .trimmingCharacters(in: .whitespacesAndNewlines)
    } catch {
      return nil
    }
  }
}
