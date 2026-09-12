import Foundation

/// Mutually exclusive user-visible presentations of one `TerminalExecution`.
enum TerminalPresentationMode: String, Equatable, Sendable {
  case flow
  case raw
  case tui
}

/// Stable terminal authority identity. Presentation transitions must not mint a
/// new execution or PTY generation.
struct TerminalPresentationIdentity: Equatable, Sendable {
  let executionId: String
  let ptyGeneration: UInt64

  static let unbound = TerminalPresentationIdentity(executionId: "unbound", ptyGeneration: 1)

  var isBound: Bool { executionId != "unbound" }
}

/// Who may admit native input after the presentation fence.
enum PresentationInputRoute: Equatable, Sendable {
  case composer
  case directTerminal
  case frozen
}

/// What the Pane Metal compositor is allowed to expose for the active mode.
struct RendererPresentationPlan: Equatable, Sendable {
  var mode: TerminalPresentationMode
  var drawsFullGridBackground: Bool
  var drawsLiveGrid: Bool
  var drawsCursorOutsideBlockRegions: Bool

  static func flow() -> RendererPresentationPlan {
    RendererPresentationPlan(
      mode: .flow,
      drawsFullGridBackground: false,
      drawsLiveGrid: false,
      drawsCursorOutsideBlockRegions: false
    )
  }

  static func fullPane(_ mode: TerminalPresentationMode) -> RendererPresentationPlan {
    RendererPresentationPlan(
      mode: mode,
      drawsFullGridBackground: true,
      drawsLiveGrid: true,
      drawsCursorOutsideBlockRegions: true
    )
  }
}

/// Observable renderer submission contract for mode flags.
struct RendererPresentationInspection: Equatable, Sendable {
  var mode: TerminalPresentationMode
  var drawsFullGridBackground: Bool
  var drawsLiveGrid: Bool
  var drawsCursorOutsideBlockRegions: Bool
  var blockRegionIDs: [UInt64]
}

/// Flow paint evidence. XCUI cannot read Metal glyphs as AX text, so tests
/// and the recovery accessibility value consume this instead of screenshots.
struct FlowPaintInspection: Equatable, Sendable {
  var mode: TerminalPresentationMode
  var liveGridSubmitted: Bool
  var fullGridBackgroundSubmitted: Bool
  var historyInstanceCount: Int
  var instancesOutsideClips: Int
  var opaquePixelsOutsideClips: Int
  var opaquePixelsInsideClips: Int

  var isClean: Bool {
    if mode != .flow {
      return true
    }
    return !liveGridSubmitted
      && !fullGridBackgroundSubmitted
      && instancesOutsideClips == 0
      && opaquePixelsOutsideClips == 0
  }

  var accessibilityToken: String {
    mode == .flow ? (isClean ? "ok" : "leak") : "n/a"
  }
}

/// Pane-owned presentation/input epoch. One session maps to one surface/PTY.
struct PanePresentationSession: Equatable, Sendable {
  private(set) var mode: TerminalPresentationMode
  private(set) var identity: TerminalPresentationIdentity
  private(set) var epoch: UInt64
  private(set) var inputRoute: PresentationInputRoute
  private(set) var lastTransitionWasExplicit: Bool

  init(
    identity: TerminalPresentationIdentity = .unbound,
    mode: TerminalPresentationMode = .raw
  ) {
    self.identity = identity
    self.mode = mode
    epoch = 1
    lastTransitionWasExplicit = false
    inputRoute = Self.route(for: mode)
  }

  var allowsDirectTerminalFirstResponder: Bool {
    mode != .flow && inputRoute == .directTerminal
  }

  var allowsEmptyCanvasTerminalHitTest: Bool {
    allowsDirectTerminalFirstResponder
  }

  var rendererPlan: RendererPresentationPlan {
    switch mode {
    case .flow: .flow()
    case .raw, .tui: .fullPane(mode)
    }
  }

  /// Bind the first observed ExecutionId/PTY without changing mode.
  mutating func bindIdentity(_ identity: TerminalPresentationIdentity) {
    guard identity.ptyGeneration != 0 else { return }
    if self.identity == .unbound || self.identity.executionId == "unbound" {
      self.identity = identity
      return
    }
    if self.identity == identity { return }
  }

  /// Atomic presentation fence. Rejects a different ExecutionId/PTY generation
  /// so a transition cannot create a second terminal authority.
  @discardableResult
  mutating func transition(
    to next: TerminalPresentationMode,
    identity: TerminalPresentationIdentity,
    explicit: Bool
  ) -> Bool {
    guard identity.ptyGeneration != 0 else { return false }
    if self.identity == .unbound || self.identity.executionId == "unbound" {
      self.identity = identity
    } else if self.identity.executionId != identity.executionId
      || self.identity.ptyGeneration != identity.ptyGeneration
    {
      return false
    }
    if next == mode {
      lastTransitionWasExplicit = explicit
      return true
    }
    inputRoute = .frozen
    epoch &+= 1
    mode = next
    lastTransitionWasExplicit = explicit
    inputRoute = Self.route(for: next)
    return true
  }

  private static func route(for mode: TerminalPresentationMode) -> PresentationInputRoute {
    switch mode {
    case .flow: .composer
    case .raw, .tui: .directTerminal
    }
  }
}

/// Bounded Flow live-tail projection policy. Running Blocks request
/// `startLine…openEndedTail`; completed Blocks use their trusted end anchor.
/// The Pane compositor still draws only registered Block clips — never a
/// Pane-wide live grid under Flow.
enum FlowLiveTailProjection {
  /// Open-ended primary history end. Runtime truncates/pages the wire range.
  static let openEndedTail: UInt64 = .max

  struct HistoryRequest: Equatable, Sendable {
    let blockID: UInt64
    let startLine: UInt64
    let endLine: UInt64
    let kind: Kind

    enum Kind: Equatable, Sendable {
      case liveTail
      case completed
    }
  }

  /// Build the history requests for one timeline snapshot.
  /// Completed IDs already present in `requestedCompleted` are skipped.
  /// Running Blocks always produce a live-tail request when eligible.
  static func historyRequests(
    records: [NativeBlockRecord],
    requestedCompleted: Set<UInt64>
  ) -> [HistoryRequest] {
    var requests: [HistoryRequest] = []
    for record in records {
      guard record.id != 0, record.startLine != 0 else { continue }
      switch record.state {
      case .running:
        requests.append(
          HistoryRequest(
            blockID: record.id,
            startLine: record.startLine,
            endLine: openEndedTail,
            kind: .liveTail
          )
        )
      case .completed:
        guard let endLine = record.endLine, endLine >= record.startLine else { continue }
        if requestedCompleted.contains(record.id) { continue }
        requests.append(
          HistoryRequest(
            blockID: record.id,
            startLine: record.startLine,
            endLine: endLine,
            kind: .completed
          )
        )
      }
    }
    return requests
  }

  /// Damage-driven live-tail refresh. Flow only; one request per generation.
  static func shouldRefreshLiveTail(
    mode: TerminalPresentationMode,
    previousGeneration: UInt64?,
    frameGeneration: UInt64,
    hasRunningBlock: Bool,
    liveTailInFlight: Bool
  ) -> Bool {
    guard mode == .flow, hasRunningBlock, !liveTailInFlight, frameGeneration != 0 else {
      return false
    }
    guard let previousGeneration else { return true }
    return frameGeneration != previousGeneration
  }

  /// History status that must fail closed (no full-grid fallback).
  static func acceptsHistoryStatus(_ status: UInt32) -> Bool {
    // 0 = ok, 1 = truncated page continuation (still usable via merge).
    status == 0 || status == 1
  }
}
