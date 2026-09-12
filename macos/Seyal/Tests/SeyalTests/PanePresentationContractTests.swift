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
  func testFlowAccessibilityPublishesPaintTokenForXCUI() {
    let surface = InteractiveMetalSurfaceView(
      frame: NSRect(x: 0, y: 0, width: 320, height: 180),
      paneID: "flow-paint-ax",
      installation: .nativeInteractionProbe
    )
    XCTAssertTrue(
      surface.applyPresentationMode(.flow, identity: identity, explicit: true)
    )
    let value = String(describing: surface.accessibilityValue() ?? "")
    XCTAssertTrue(
      value.contains("flow-paint=ok"),
      "XCUI reads Metal paint through recovery accessibility, not glyph text: \(value)"
    )
  }

  @MainActor
  func testFlowOffscreenPaintDoesNotSubmitLiveGridOntoEmptyCanvas() throws {
    guard let device = MTLCreateSystemDefaultDevice() else {
      throw XCTSkip("Metal device unavailable in this environment")
    }
    let renderer = try MetalTerminalRenderer(
      device: device,
      terminalFont: .canonicalTerminal
    )
    renderer.setPresentationPlan(.flow())
    var damage = DamageMask()
    damage.markAll(rows: 2)
    let cells = [
      preparedCell(scalar: 65, background: 0x02FF_0000),
      preparedCell(scalar: 66, background: 0x0200_FF00),
      preparedCell(scalar: 67, background: 0x0200_00FF),
      preparedCell(scalar: 68, background: 0x02FF_FF00),
    ]
    XCTAssertEqual(
      try renderer.update(
        frame: NativePreparedFrame(
          cells: cells,
          generation: 1,
          rows: 2,
          columns: 2,
          damage: damage
        ),
        backingScale: 1,
        forceFullRebuild: true
      ),
      .updated
    )
    let cellSize = renderer.cellPixelSize(backingScale: 1)
    let texture = try XCTUnwrap(
      renderer.renderOffscreenAndWait(
        width: cellSize.width * 2,
        height: cellSize.height * 2
      )
    )
    let paint = renderer.inspectFlowPaint(from: texture)
    XCTAssertFalse(paint.liveGridSubmitted)
    XCTAssertFalse(paint.fullGridBackgroundSubmitted)
    XCTAssertEqual(paint.opaquePixelsOutsideClips, 0)
    XCTAssertEqual(paint.opaquePixelsInsideClips, 0)
    XCTAssertTrue(paint.isClean)
  }

  @MainActor
  func testFlowHistoryPreparationDropsInstancesOutsideBlockClip() throws {
    guard let device = MTLCreateSystemDefaultDevice() else {
      throw XCTSkip("Metal device unavailable in this environment")
    }
    let renderer = try MetalTerminalRenderer(
      device: device,
      terminalFont: .canonicalTerminal
    )
    renderer.setPresentationPlan(.flow())
    let cellSize = renderer.cellPixelSize(backingScale: 1)
    let history = NativeHistoryRange(
      startLine: 1,
      endLine: 1,
      blockID: 11,
      requestID: 1,
      revision: 1,
      rows: [[
        NativeHistoryRange.Cell(
          scalar: 65,
          foreground: 0xffe9_e1d8,
          background: 0xff10_0d0b,
          flags: 0
        )
      ]]
    )
    let clip = NSRect(
      x: CGFloat(cellSize.width),
      y: 0,
      width: CGFloat(cellSize.width),
      height: CGFloat(cellSize.height)
    )
    XCTAssertEqual(
      try renderer.update(
        historyRange: history,
        region: NativeTranscriptRegion(id: 11, origin: .zero, clip: clip),
        backingScale: 1
      ),
      .updated
    )
    renderer.setHistoryRegionOrder([11])
    let clipped = renderer.inspectFlowPaint()
    XCTAssertEqual(clipped.historyInstanceCount, 0)
    XCTAssertEqual(clipped.instancesOutsideClips, 0)
    XCTAssertTrue(clipped.isClean)
    XCTAssertEqual(clipped.accessibilityToken, "ok")

    XCTAssertEqual(
      try renderer.update(
        historyRange: history,
        region: NativeTranscriptRegion(id: 11, origin: clip.origin, clip: clip),
        backingScale: 1
      ),
      .updated
    )
    let texture = try XCTUnwrap(
      renderer.renderOffscreenAndWait(
        width: cellSize.width * 2,
        height: cellSize.height
      )
    )
    let confined = renderer.inspectFlowPaint(from: texture)
    XCTAssertEqual(confined.instancesOutsideClips, 0)
    XCTAssertEqual(confined.opaquePixelsOutsideClips, 0)
    XCTAssertGreaterThan(confined.opaquePixelsInsideClips, 0)
    XCTAssertTrue(confined.isClean)
  }

  @MainActor
  func testFlowBlockBodyRegionSpansLeadingCanvasNotTrailingStrip() {
    let visual = SeyalThemeResolver.canonical(.dark)
    let host = NSView(frame: NSRect(x: 0, y: 0, width: 720, height: 420))
    host.translatesAutoresizingMaskIntoConstraints = false
    let stack = TranscriptBlockStackView()
    stack.orientation = .vertical
    stack.alignment = .width
    stack.translatesAutoresizingMaskIntoConstraints = false
    let body = CommandBlockBodyView()
    let block = BlockView(
      presentation: BlockPresentation(
        id: "11",
        command: "ls /",
        state: .completed,
        elapsed: "Done",
        timestamp: nil,
        isSelected: false,
        actions: []
      ),
      bodyView: body,
      visual: visual
    )
    host.addSubview(stack)
    stack.addArrangedSubview(block)
    NSLayoutConstraint.activate([
      stack.leadingAnchor.constraint(equalTo: host.leadingAnchor, constant: 8),
      stack.trailingAnchor.constraint(equalTo: host.trailingAnchor, constant: -8),
      stack.topAnchor.constraint(equalTo: host.topAnchor, constant: 8),
      block.widthAnchor.constraint(equalTo: stack.widthAnchor),
      host.widthAnchor.constraint(equalToConstant: 720),
      host.heightAnchor.constraint(equalToConstant: 420),
    ])
    host.layoutSubtreeIfNeeded()

    XCTAssertGreaterThan(
      body.bounds.width,
      480,
      "Block body must span the Flow canvas; a trailing strip is the screenshot leak"
    )
    XCTAssertLessThan(
      body.frame.minX + stack.frame.minX,
      48,
      "Block body must start at the leading canvas, not a trailing terminal column"
    )
  }

  func testLiveTailRequestsOpenEndedRangeForRunningBlocks() {
    let running = NativeBlockRecord(
      id: 7,
      command: "seq 1 1000",
      state: .running,
      startLine: 11,
      endLine: nil,
      exitStatus: 0
    )
    let completed = NativeBlockRecord(
      id: 3,
      command: "printf hello",
      state: .completed,
      startLine: 1,
      endLine: 2,
      exitStatus: 0
    )
    let first = FlowLiveTailProjection.historyRequests(
      records: [completed, running],
      requestedCompleted: []
    )
    XCTAssertEqual(
      first,
      [
        .init(blockID: 3, startLine: 1, endLine: 2, kind: .completed),
        .init(
          blockID: 7,
          startLine: 11,
          endLine: FlowLiveTailProjection.openEndedTail,
          kind: .liveTail
        ),
      ]
    )
    let again = FlowLiveTailProjection.historyRequests(
      records: [completed, running],
      requestedCompleted: [3]
    )
    XCTAssertEqual(
      again,
      [
        .init(
          blockID: 7,
          startLine: 11,
          endLine: FlowLiveTailProjection.openEndedTail,
          kind: .liveTail
        )
      ]
    )
  }

  func testLiveTailRefreshIsGenerationCoalescedInFlowOnly() {
    XCTAssertTrue(
      FlowLiveTailProjection.shouldRefreshLiveTail(
        mode: .flow,
        previousGeneration: nil,
        frameGeneration: 1,
        hasRunningBlock: true,
        liveTailInFlight: false
      )
    )
    XCTAssertFalse(
      FlowLiveTailProjection.shouldRefreshLiveTail(
        mode: .flow,
        previousGeneration: 4,
        frameGeneration: 4,
        hasRunningBlock: true,
        liveTailInFlight: false
      )
    )
    XCTAssertTrue(
      FlowLiveTailProjection.shouldRefreshLiveTail(
        mode: .flow,
        previousGeneration: 4,
        frameGeneration: 5,
        hasRunningBlock: true,
        liveTailInFlight: false
      )
    )
    XCTAssertFalse(
      FlowLiveTailProjection.shouldRefreshLiveTail(
        mode: .flow,
        previousGeneration: 4,
        frameGeneration: 5,
        hasRunningBlock: true,
        liveTailInFlight: true
      )
    )
    XCTAssertFalse(
      FlowLiveTailProjection.shouldRefreshLiveTail(
        mode: .raw,
        previousGeneration: 4,
        frameGeneration: 5,
        hasRunningBlock: true,
        liveTailInFlight: false
      )
    )
  }

  func testLiveTailRejectsUnsafeHistoryStatus() {
    XCTAssertTrue(FlowLiveTailProjection.acceptsHistoryStatus(0))
    XCTAssertTrue(FlowLiveTailProjection.acceptsHistoryStatus(1))
    XCTAssertFalse(FlowLiveTailProjection.acceptsHistoryStatus(2))
  }

  func testCompletionHandoffReplacesOpenEndedLiveTailWithTrustedEnd() {
    let running = NativeBlockRecord(
      id: 7,
      command: "seq 1 1000",
      state: .running,
      startLine: 11,
      endLine: nil,
      exitStatus: 0
    )
    let completed = NativeBlockRecord(
      id: 7,
      command: "seq 1 1000",
      state: .completed,
      startLine: 11,
      endLine: 42,
      exitStatus: 0
    )
    XCTAssertEqual(
      FlowLiveTailProjection.historyRequests(records: [running], requestedCompleted: []),
      [
        .init(
          blockID: 7,
          startLine: 11,
          endLine: FlowLiveTailProjection.openEndedTail,
          kind: .liveTail
        )
      ]
    )
    XCTAssertEqual(
      FlowLiveTailProjection.historyRequests(records: [completed], requestedCompleted: []),
      [
        .init(blockID: 7, startLine: 11, endLine: 42, kind: .completed)
      ]
    )
  }

  @MainActor
  func testLiveTailHistoryGrowthStaysInsideBlockClipWithoutLiveGrid() throws {
    guard let device = MTLCreateSystemDefaultDevice() else {
      throw XCTSkip("Metal device unavailable in this environment")
    }
    let renderer = try MetalTerminalRenderer(
      device: device,
      terminalFont: .canonicalTerminal
    )
    renderer.setPresentationPlan(.flow())
    let cellSize = renderer.cellPixelSize(backingScale: 1)
    let clip = NSRect(
      x: 0,
      y: 0,
      width: CGFloat(cellSize.width) * 4,
      height: CGFloat(cellSize.height) * 3
    )
    func cell(_ scalar: UInt32) -> NativeHistoryRange.Cell {
      NativeHistoryRange.Cell(
        scalar: scalar,
        foreground: 0xffe9_e1d8,
        background: 0xff10_0d0b,
        flags: 0
      )
    }
    let first = NativeHistoryRange(
      startLine: 11,
      endLine: FlowLiveTailProjection.openEndedTail,
      blockID: 42,
      requestID: 1,
      revision: 1,
      rows: [[cell(65)]]
    )
    let grown = NativeHistoryRange(
      startLine: 11,
      endLine: FlowLiveTailProjection.openEndedTail,
      blockID: 42,
      requestID: 2,
      revision: 2,
      rows: [[cell(65)], [cell(66)], [cell(67)]]
    )
    let region = NativeTranscriptRegion(id: 42, origin: clip.origin, clip: clip)
    XCTAssertEqual(
      try renderer.update(historyRange: first, region: region, backingScale: 1),
      .updated
    )
    renderer.setHistoryRegionOrder([42])
    XCTAssertEqual(
      try renderer.update(historyRange: grown, region: region, backingScale: 1),
      .updated
    )
    let texture = try XCTUnwrap(
      renderer.renderOffscreenAndWait(
        width: cellSize.width * 8,
        height: cellSize.height * 6
      )
    )
    let paint = renderer.inspectFlowPaint(from: texture)
    XCTAssertFalse(paint.liveGridSubmitted)
    XCTAssertFalse(paint.fullGridBackgroundSubmitted)
    XCTAssertEqual(paint.instancesOutsideClips, 0)
    XCTAssertEqual(paint.opaquePixelsOutsideClips, 0)
    XCTAssertGreaterThan(paint.opaquePixelsInsideClips, 0)
    XCTAssertTrue(paint.isClean)
  }

  @MainActor
  private func descendants<T: NSView>(of type: T.Type, in root: NSView) -> [T] {
    root.subviews.flatMap { child -> [T] in
      var matches = child is T ? [child as! T] : []
      matches.append(contentsOf: descendants(of: type, in: child))
      return matches
    }
  }

  private func preparedCell(scalar: UInt32, background: UInt32) -> SeyalPreparedCell {
    var cell = SeyalPreparedCell()
    cell.scalar = scalar
    cell.foreground = 0x02E9_E1D8
    cell.background = background
    cell.flags = 0
    cell.reserved = 0
    return cell
  }
}
