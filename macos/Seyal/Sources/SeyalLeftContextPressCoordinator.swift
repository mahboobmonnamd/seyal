import AppKit

/// Ephemeral press-preview identity for left-context rows and top tab chips.
enum LeftPressPreview: Sendable, Equatable {
  case workspace(String)
  case agent(String)
  case tab(String)
}

struct ContextRowVisuals: Sendable {
  let container: NSView
  let button: NSButton
}

struct TabChipVisuals: Sendable {
  let container: NSView
  let button: NSButton
  let accentView: NSView
}

@MainActor
enum LeftContextRowEmphasis {
  static func apply(
    to visuals: ContextRowVisuals,
    emphasized: Bool,
    visual: SeyalResolvedVisualConfiguration
  ) {
    visuals.container.wantsLayer = true
    visuals.container.layer?.backgroundColor = emphasized
      ? visual.colors.ns(.selection).cgColor
      : NSColor.clear.cgColor

    visuals.button.font = emphasized
      ? visual.typography[.action]
      : visual.typography[.uiBody]
    visuals.button.contentTintColor = emphasized
      ? visual.colors.ns(.textPrimary)
      : visual.colors.ns(.textSecondary)
  }

  static func applyTabChip(
    to visuals: TabChipVisuals,
    emphasized: Bool,
    visual: SeyalResolvedVisualConfiguration
  ) {
    visuals.container.wantsLayer = true
    visuals.container.layer?.backgroundColor = emphasized
      ? visual.colors.ns(.utilityActive).cgColor
      : NSColor.clear.cgColor

    visuals.button.font = emphasized
      ? visual.typography[.windowTitle]
      : visual.typography[.tab]
    visuals.button.contentTintColor = emphasized
      ? visual.colors.ns(.textPrimary)
      : visual.colors.ns(.textSecondary)
    visuals.accentView.isHidden = !emphasized
  }
}

/// Owns ephemeral left-context press-preview state and live row visual registries.
/// Does not mutate authoritative selection; commit delegates to `SeyalShellState`.
@MainActor
final class SeyalLeftContextPressCoordinator {
  private(set) var preview: LeftPressPreview?

  private(set) var workspaceRowVisualsByID: [String: ContextRowVisuals] = [:]
  private(set) var agentRowVisualsByID: [String: ContextRowVisuals] = [:]
  private(set) var leftTabRowVisualsByID: [String: ContextRowVisuals] = [:]
  private(set) var topTabChipVisualsByID: [String: TabChipVisuals] = [:]

  func reset() {
    preview = nil
    workspaceRowVisualsByID.removeAll()
    agentRowVisualsByID.removeAll()
    leftTabRowVisualsByID.removeAll()
    topTabChipVisualsByID.removeAll()
  }

  func registerRow(_ visuals: ContextRowVisuals, kind: LeftPressPreview, itemID: String) {
    switch kind {
    case .workspace:
      workspaceRowVisualsByID[itemID] = visuals
    case .agent:
      agentRowVisualsByID[itemID] = visuals
    case .tab:
      leftTabRowVisualsByID[itemID] = visuals
    }
  }

  func registerTabChip(_ visuals: TabChipVisuals, tabID: String) {
    topTabChipVisualsByID[tabID] = visuals
  }

  func applyStyles(
    committedWorkspaceID: String,
    committedAgentID: String?,
    committedTabID: String,
    visual: SeyalResolvedVisualConfiguration
  ) {
    for (id, visuals) in workspaceRowVisualsByID {
      let emphasized: Bool
      if case let .workspace(previewID) = preview {
        emphasized = id == previewID
      } else {
        emphasized = id == committedWorkspaceID
      }
      LeftContextRowEmphasis.apply(to: visuals, emphasized: emphasized, visual: visual)
    }

    for (id, visuals) in agentRowVisualsByID {
      let emphasized: Bool
      if case let .agent(previewID) = preview {
        emphasized = id == previewID
      } else {
        emphasized = id == committedAgentID
      }
      LeftContextRowEmphasis.apply(to: visuals, emphasized: emphasized, visual: visual)
    }

    for (id, visuals) in leftTabRowVisualsByID {
      let emphasized: Bool
      if case let .tab(previewID) = preview {
        emphasized = id == previewID
      } else {
        emphasized = id == committedTabID
      }
      LeftContextRowEmphasis.apply(to: visuals, emphasized: emphasized, visual: visual)
    }

    for (id, visuals) in topTabChipVisualsByID {
      let emphasized: Bool
      if case let .tab(previewID) = preview {
        emphasized = id == previewID
      } else {
        emphasized = id == committedTabID
      }
      LeftContextRowEmphasis.applyTabChip(to: visuals, emphasized: emphasized, visual: visual)
    }
  }

  func begin(_ next: LeftPressPreview) {
    preview = next
  }

  func cancel() {
    preview = nil
  }

  func commit(_ committed: LeftPressPreview, state: SeyalShellState) {
    preview = nil
    switch committed {
    case .workspace(let id):
      state.selectWorkspace(id: id)
    case .agent(let id):
      state.selectAgent(id: id)
    case .tab(let id):
      state.selectTab(id: id)
    }
  }
}
