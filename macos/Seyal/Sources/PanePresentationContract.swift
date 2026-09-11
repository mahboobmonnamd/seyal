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

/// Observable renderer submission contract. Tests assert this rather than
/// sampling GPU pixels.
struct RendererPresentationInspection: Equatable, Sendable {
  var mode: TerminalPresentationMode
  var drawsFullGridBackground: Bool
  var drawsLiveGrid: Bool
  var drawsCursorOutsideBlockRegions: Bool
  var blockRegionIDs: [UInt64]
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
