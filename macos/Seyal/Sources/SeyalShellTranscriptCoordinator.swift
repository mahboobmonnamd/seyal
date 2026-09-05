import AppKit

extension SeyalShellView {
  static func retainedHistoryKeys(
    existing: [PaneBlockKey],
    paneID: String,
    retainedBlockIDs: [UInt64]
  ) -> [PaneBlockKey] {
    let retained = Set(retainedBlockIDs)
    return existing.filter { key in
      key.paneID != paneID || retained.contains(key.blockID)
    }
  }
  @discardableResult
  func submitCommand(_ command: String, paneID: String) -> Bool {
    guard let surface = surfaces[paneID], surface.ensureTerminalBridgeConnected() else {
      return false
    }
    let requestID = surface.terminalNextComposerRequestID()
    guard requestID != 0, surface.terminalSubmitComposerCommand(command) == 0
    else { return false }
    // Runtime owns Block identity/lifecycle. Keep the draft until the
    // authoritative request-correlated result reports acceptance.
    pendingComposerRequests[paneID] = requestID
    return true
  }

  /// Reconciles only the transcript block stack. Pane-owned composer and
  /// bridge-backed Metal surface instances survive timeline revisions.
  func updateTranscriptBlocks(paneID: String) {
    guard productionShell,
      let transcript = transcriptDocuments[paneID],
      let surface = surfaces[paneID],
      let paneState = state.activeTab.panes[paneID]
    else { return }
    let blockIDs = paneState.blocks.map(\.id)
    let blocks = paneState.blocks
    let blockKeys = blocks.compactMap { item -> PaneBlockKey? in
      guard let blockID = UInt64(item.id) else { return nil }
      return PaneBlockKey(paneID: paneID, blockID: blockID)
    }
    renderedBlockIDs[paneID] = blockKeys
    transcript.unregisterMissingBlockBodies(Set(blocks.compactMap { UInt64($0.id) }))
    let stack: NSStackView
    if let existing = blockStacks[paneID] {
      stack = existing
    } else {
      stack = NSStackView()
      stack.orientation = .vertical
      stack.alignment = .leading
      stack.spacing = 8
      stack.translatesAutoresizingMaskIntoConstraints = false
      blockStacks[paneID] = stack
      transcript.installBlockStack(stack)
    }
    let retainedKeys = Set(blockKeys)
    for oldKey in Set(blockViews.keys).filter({ $0.paneID == paneID }).subtracting(retainedKeys) {
      if let view = blockViews.removeValue(forKey: oldKey) {
        stack.removeArrangedSubview(view)
        view.removeFromSuperview()
      }
      blockConstraintOwnership.remove(oldKey)
      blockBodies.removeValue(forKey: oldKey)
    }
    var orderedViews: [BlockView] = []
    for (index, item) in blocks.enumerated() {
      guard let blockID = UInt64(item.id) else { continue }
      let key = PaneBlockKey(paneID: paneID, blockID: blockID)
      let body =
        blockBodies[key]
        ?? {
          let body = CommandBlockBodyView()
          blockBodies[key] = body
          return body
        }()
      let block =
        blockViews[key]
        ?? {
          let block = BlockView(
            presentation: BlockPresentation(
              id: item.id,
              command: item.command,
              state: item.state,
              elapsed: index == blocks.count - 1 ? "Live" : "Done",
              timestamp: nil,
              isSelected: index == blocks.count - 1,
              actions: []
            ),
            bodyView: body,
            visual: visual
          )
          block.setAccessibilityIdentifier(key.accessibilityIdentifier)
          blockViews[key] = block
          return block
        }()
      block.apply(
        presentation: BlockPresentation(
          id: item.id,
          command: item.command,
          state: item.state,
          elapsed: index == blocks.count - 1 ? "Live" : "Done",
          timestamp: nil,
          isSelected: index == blocks.count - 1,
          actions: []
        ))
      if index == blocks.count - 1 { tuiBlocks[paneID] = block }
      transcript.registerBlockBody(body, blockID: blockID)
      orderedViews.append(block)
    }
    // NSStackView has no keyed diff API. Removing/re-adding arranged
    // subviews preserves each BlockView/body object and its layer pixels.
    for view in stack.arrangedSubviews {
      stack.removeArrangedSubview(view)
      view.removeFromSuperview()
    }
    for (index, view) in orderedViews.enumerated() {
      stack.addArrangedSubview(view)
      guard let blockID = UInt64(blocks[index].id) else { continue }
      let key = PaneBlockKey(paneID: paneID, blockID: blockID)
      if !blockConstraintOwnership.contains(key) {
        let constraint = view.widthAnchor.constraint(equalTo: stack.widthAnchor)
        blockConstraintOwnership.install([constraint], for: key)
      }
    }
    if let latest = orderedViews.last {
      tuiBlocks[paneID] = latest
    } else {
      tuiBlocks.removeValue(forKey: paneID)
    }
    let nativeBlockIDs = blockIDs.compactMap { UInt64($0) }
    transcript.replaceBlockOrder(nativeBlockIDs)
    surface.removeTranscriptRegions(except: Set(nativeBlockIDs))
    surface.setTranscriptFrame(transcript.transcriptFrame())
    surface.publishCurrentTerminalFrame()
  }

  /// Each Pane owns one composer and a Runtime-projected command Block
  /// timeline. The bridge surface remains the canonical PTY/VT source.
  func makeTranscript(paneID: String) -> PaneTranscriptView {
    let paneState = state.activeTab.panes[paneID]
    let transcript = PaneTranscriptView(
      paneID: paneID,
      installSurface: productionShell,
      executionIdentity: paneState?.executionIdentity,
      allowsImplicitExecutionBootstrap: paneState?.allowsImplicitExecutionBootstrap ?? false,
      visual: visual
    )
    transcript.setAccessibilityIdentifier("transcript.\(paneID)")
    let document = transcript.transcriptDocument
    let surface = transcript.terminalSurface

    if productionShell {
      surface.setAccessibilityIdentifier("terminal-surface.\(paneID)")
      surface.onAlternateScreenChanged = { [weak self] active in
        self?.setPaneTUI(paneID: paneID, active: active)
      }
      surface.onTimelineChanged = { [weak self] records in
        guard let self else { return }
        let retainedBlockKeys = Set(
          records.map {
            PaneBlockKey(paneID: paneID, blockID: $0.id)
          })
        self.requestedHistoryBlocks = Self.retainedHistoryKeys(
          existing: Array(self.requestedHistoryBlocks),
          paneID: paneID,
          retainedBlockIDs: records.map(\.id)
        ).reduce(into: Set<PaneBlockKey>()) { $0.insert($1) }
        surface.discardHistoryRequests(except: Set(retainedBlockKeys.map(\.blockID)))
        self.state.applyRuntimeBlocks(records, paneID: paneID)
        self.updateTranscriptBlocks(paneID: paneID)
        for record in records
        where record.state == .completed
          && record.endLine != nil
          && !self.requestedHistoryBlocks.contains(
            PaneBlockKey(paneID: paneID, blockID: record.id)
          )
        {
          self.requestedHistoryBlocks.insert(
            PaneBlockKey(paneID: paneID, blockID: record.id)
          )
          _ = surface.requestHistoryRange(
            startLine: record.startLine,
            endLine: record.endLine ?? record.startLine,
            blockID: record.id
          )
        }
        if let latest = records.last {
          self.composerViews[paneID]?.setBusy(
            latest.state == .running,
            process: latest.command
          )
        }
      }
      surface.onHistoryRangeChanged = { [weak self] range in
        let key = PaneBlockKey(paneID: paneID, blockID: range.blockID)
        guard let self,
          let body = self.blockBodies[key]
        else { return }
        body.setHistoryRange(range)
        let region = transcript.region(for: range.blockID)
        surface.renderHistoryRange(range, region: region)
      }
      surface.onComposerResultChanged = { [weak self] result in
        guard let self, self.pendingComposerRequests[paneID] == result.requestID else {
          return
        }
        self.pendingComposerRequests.removeValue(forKey: paneID)
        switch result.code {
        case .accepted:
          self.state.updateDraft("", paneID: paneID)
          self.composerViews[paneID]?.clearAcceptedDraft()
        case .busy, .unsupported, .backpressure, .invalid:
          self.composerViews[paneID]?.setBusy(false, process: "")
        }
      }
      surfaces[paneID] = surface
      transcriptDocuments[paneID] = transcript
      if paneState?.executionIdentity == nil,
        paneState?.allowsImplicitExecutionBootstrap == true,
        let identity = surface.terminalExecutionIdentity
      {
        state.bindExecutionIdentity(identity, paneID: paneID)
      }
      updateTranscriptBlocks(paneID: paneID)
    } else {
      let host = TerminalSurfaceHostView(frame: .zero)
      host.translatesAutoresizingMaskIntoConstraints = false
      document.addSubview(host)

      let title = NSTextField(labelWithString: "No TerminalExecution attached")
      title.font = visual.typography[.action]
      title.textColor = visual.colors.ns(.textSecondary)
      title.alignment = .center

      let detail = NSTextField(
        labelWithString: "UI preview only · terminal authority remains unwired until Pass 6")
      detail.font = visual.typography[.metadata]
      detail.textColor = visual.colors.ns(.textMuted)
      detail.alignment = .center

      let empty = NSStackView(views: [title, detail])
      empty.orientation = .vertical
      empty.alignment = .centerX
      empty.spacing = 4
      empty.translatesAutoresizingMaskIntoConstraints = false
      document.addSubview(empty)

      NSLayoutConstraint.activate([
        host.leadingAnchor.constraint(equalTo: document.leadingAnchor),
        host.trailingAnchor.constraint(equalTo: document.trailingAnchor),
        host.topAnchor.constraint(equalTo: document.topAnchor),
        host.bottomAnchor.constraint(equalTo: document.bottomAnchor),
        empty.centerXAnchor.constraint(equalTo: document.centerXAnchor),
        empty.centerYAnchor.constraint(equalTo: document.centerYAnchor),
      ])
    }

    return transcript
  }
  @objc
  func reconnectPane(_ sender: NSButton) {
    guard let paneID = sender.identifier?.rawValue,
      let surface = surfaces[paneID]
    else { return }
    state.focusPane(id: paneID)
    _ = surface.retryRuntimeConnection()
  }
}
