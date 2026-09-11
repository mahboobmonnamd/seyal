import AppKit
import Metal
import XCTest
@testable import Seyal

final class PanePresentationContractTests: XCTestCase {
  private let identity = TerminalPresentationIdentity(
    executionId: "exec-1",
    ptyGeneration: 7
  )

  func testFlowRawTuiAreMutuallyExclusiveModes() {
    var session = PanePresentationSession(identity: identity, mode: .flow)
    XCTAssertEqual(session.mode, .flow)
    XCTAssertEqual(session.inputRoute, .composer)
    XCTAssertFalse(session.allowsDirectTerminalFirstResponder)
    XCTAssertFalse(session.allowsEmptyCanvasTerminalHitTest)

    XCTAssertTrue(session.transition(to: .raw, identity: identity, explicit: true))
    XCTAssertEqual(session.mode, .raw)
    XCTAssertNotEqual(session.mode, .flow)
    XCTAssertNotEqual(session.mode, .tui)
    XCTAssertEqual(session.inputRoute, .directTerminal)

    XCTAssertTrue(session.transition(to: .tui, identity: identity, explicit: false))
    XCTAssertEqual(session.mode, .tui)
    XCTAssertNotEqual(session.mode, .flow)
    XCTAssertNotEqual(session.mode, .raw)
  }

  func testFlowToTuiToFlowKeepsExecutionAndPtyIdentity() {
    var session = PanePresentationSession(identity: identity, mode: .flow)
    let startEpoch = session.epoch
    XCTAssertTrue(session.transition(to: .tui, identity: identity, explicit: false))
    XCTAssertEqual(session.identity, identity)
    XCTAssertGreaterThan(session.epoch, startEpoch)
    XCTAssertTrue(session.transition(to: .flow, identity: identity, explicit: false))
    XCTAssertEqual(session.mode, .flow)
    XCTAssertEqual(session.identity.executionId, "exec-1")
    XCTAssertEqual(session.identity.ptyGeneration, 7)
    XCTAssertEqual(session.inputRoute, .composer)
  }

  func testFlowToRawToFlowRequiresExplicitNamedTransition() {
    var session = PanePresentationSession(identity: identity, mode: .flow)
    XCTAssertFalse(session.lastTransitionWasExplicit)
    XCTAssertTrue(session.transition(to: .raw, identity: identity, explicit: true))
    XCTAssertTrue(session.lastTransitionWasExplicit)
    XCTAssertEqual(session.mode, .raw)
    XCTAssertTrue(session.transition(to: .flow, identity: identity, explicit: true))
    XCTAssertTrue(session.lastTransitionWasExplicit)
    XCTAssertEqual(session.identity, identity)
  }

  func testTransitionRejectsADifferentExecutionOrPtyAuthority() {
    var session = PanePresentationSession(identity: identity, mode: .flow)
    let otherExecution = TerminalPresentationIdentity(
      executionId: "exec-2",
      ptyGeneration: 7
    )
    let otherPty = TerminalPresentationIdentity(
      executionId: "exec-1",
      ptyGeneration: 8
    )
    XCTAssertFalse(session.transition(to: .tui, identity: otherExecution, explicit: false))
    XCTAssertEqual(session.mode, .flow)
    XCTAssertFalse(session.transition(to: .raw, identity: otherPty, explicit: true))
    XCTAssertEqual(session.mode, .flow)
    XCTAssertEqual(session.identity, identity)
  }

  func testFlowRendererPlanNeverExposesFullGridOrCursorOutsideBlocks() {
    let plan = RendererPresentationPlan.flow()
    XCTAssertFalse(plan.drawsFullGridBackground)
    XCTAssertFalse(plan.drawsLiveGrid)
    XCTAssertFalse(plan.drawsCursorOutsideBlockRegions)
    let raw = RendererPresentationPlan.fullPane(.raw)
    XCTAssertTrue(raw.drawsFullGridBackground)
    XCTAssertTrue(raw.drawsLiveGrid)
    XCTAssertTrue(raw.drawsCursorOutsideBlockRegions)
  }

  @MainActor
  func testFlowSurfaceCannotBecomeFirstResponder() {
    let surface = InteractiveMetalSurfaceView(
      frame: NSRect(x: 0, y: 0, width: 320, height: 180),
      paneID: "flow-contract",
      installation: .nativeInteractionProbe
    )
    XCTAssertTrue(
      surface.applyPresentationMode(.flow, identity: identity, explicit: true)
    )
    XCTAssertFalse(surface.claimsFirstResponderOnClick)
    XCTAssertFalse(surface.acceptsFirstResponder)
    XCTAssertFalse(surface.becomeFirstResponder())
  }

  @MainActor
  func testEmptyFlowTranscriptDoesNotRouteHitsToTerminalSurface() {
    let transcript = PaneTranscriptView(visual: SeyalThemeResolver.canonical(.dark))
    transcript.frame = NSRect(x: 0, y: 0, width: 720, height: 420)
    let stack = TranscriptBlockStackView()
    stack.orientation = .vertical
    stack.alignment = .width
    transcript.installBlockStack(stack)
    transcript.layoutSubtreeIfNeeded()

    XCTAssertEqual(transcript.terminalSurface.presentation.mode, .flow)
    XCTAssertFalse(transcript.terminalSurface.allowsEmptyCanvasTerminalHitTest)
    let emptyPoint = NSPoint(x: 24, y: 24)
    let hit = transcript.hitTest(emptyPoint)
    XCTAssertFalse(
      hit === transcript.terminalSurface,
      "empty Flow canvas must not hit-test into the Metal terminal surface"
    )
  }

  @MainActor
  func testFlowRendererInspectionOmitsFullGridBackgroundAndCursor() throws {
    guard let device = MTLCreateSystemDefaultDevice() else {
      throw XCTSkip("Metal device unavailable in this environment")
    }
    let renderer = try MetalTerminalRenderer(
      device: device,
      terminalFont: .canonicalTerminal
    )
    renderer.setPresentationPlan(.flow())
    renderer.setHistoryRegionOrder([11, 12])
    let inspection = renderer.inspectPresentation()
    XCTAssertEqual(inspection.mode, .flow)
    XCTAssertFalse(inspection.drawsFullGridBackground)
    XCTAssertFalse(inspection.drawsLiveGrid)
    XCTAssertFalse(inspection.drawsCursorOutsideBlockRegions)
    XCTAssertEqual(inspection.blockRegionIDs, [11, 12])
  }

  @MainActor
  func testResizeAndModeTransitionsReuseOneTerminalSurface() {
    let transcript = PaneTranscriptView(visual: SeyalThemeResolver.canonical(.dark))
    let surface = transcript.terminalSurface
    XCTAssertTrue(
      surface.applyPresentationMode(.flow, identity: identity, explicit: true)
    )
    transcript.frame = NSRect(x: 0, y: 0, width: 400, height: 240)
    transcript.layoutSubtreeIfNeeded()
    XCTAssertTrue(
      surface.applyPresentationMode(.tui, identity: identity, explicit: false)
    )
    transcript.frame = NSRect(x: 0, y: 0, width: 800, height: 480)
    transcript.layoutSubtreeIfNeeded()
    XCTAssertTrue(
      surface.applyPresentationMode(.raw, identity: identity, explicit: true)
    )
    XCTAssertTrue(
      surface.applyPresentationMode(.flow, identity: identity, explicit: true)
    )
    XCTAssertTrue(transcript.terminalSurface === surface)
    XCTAssertEqual(surface.presentation.identity, identity)
    XCTAssertEqual(
      descendants(of: InteractiveMetalSurfaceView.self, in: transcript).count,
      1
    )
  }

  @MainActor
  private func descendants<T: NSView>(of type: T.Type, in root: NSView) -> [T] {
    root.subviews.flatMap { child -> [T] in
      var matches = child is T ? [child as! T] : []
      matches.append(contentsOf: descendants(of: type, in: child))
      return matches
    }
  }
}
