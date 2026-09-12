import AppKit

/// One-pane AppKit host. Product state is Rust-owned (`seyal_app_*`).
/// Terminal frames stay on the Candidate-D bridge (`seyal_bridge_*`).
@MainActor
final class ThinPaneHostView: NSView {
    let inputSurface: InteractiveMetalSurfaceView
    private let appHandle: UInt64
    private var lastBoundExecution = (low: UInt64(0), high: UInt64(0))

    override init(frame frameRect: NSRect) {
        appHandle = seyal_app_create()
        inputSurface = InteractiveMetalSurfaceView(
            frame: frameRect,
            appHandle: appHandle
        )
        super.init(frame: frameRect)
        translatesAutoresizingMaskIntoConstraints = false
        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        setAccessibilityIdentifier("seyal-thin-pane")
        inputSurface.translatesAutoresizingMaskIntoConstraints = false
        addSubview(inputSurface)
        NSLayoutConstraint.activate([
            inputSurface.leadingAnchor.constraint(equalTo: leadingAnchor),
            inputSurface.trailingAnchor.constraint(equalTo: trailingAnchor),
            inputSurface.topAnchor.constraint(equalTo: topAnchor),
            inputSurface.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
        inputSurface.onAlternateScreenChanged = { [weak self] alternate in
            self?.inputSurface.observedAlternateScreen = alternate
            self?.bindFromBridgeIfNeeded()
        }
        inputSurface.onFrameChanged = { [weak self] _ in
            self?.bindFromBridgeIfNeeded()
        }
        inputSurface.onBridgeBecameUsable = { [weak self] in
            self?.bindFromBridgeIfNeeded()
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("ThinPaneHostView is programmatic")
    }

    deinit {
        seyal_app_destroy(appHandle)
    }

    func activateAfterWindowPresentation() {
        inputSurface.activateRuntimeAfterWindowPresentation()
        bindFromBridgeIfNeeded()
        announceAccessibility()
    }

    func requestQuit() {
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_QUIT.rawValue)
        _ = seyal_app_apply(appHandle, &action)
        let snapshot = seyal_app_snapshot(appHandle)
        if snapshot.pending_effect != 0 {
            var ack = SeyalAppAction()
            ack.version = UInt16(SEYAL_APP_ABI_VERSION)
            ack.size = UInt16(MemoryLayout<SeyalAppAction>.size)
            ack.kind = UInt16(SEYAL_APP_ACTION_ACK_EFFECT.rawValue)
            _ = seyal_app_apply(appHandle, &ack)
        }
    }

    func detachForTermination() {
        inputSurface.detachRuntimeConnectionForApplicationTermination()
    }

    private func bindFromBridgeIfNeeded() {
        guard inputSurface.terminalBridgeIsConnected else { return }
        let executionLow = seyal_bridge_execution_id_low()
        let executionHigh = seyal_bridge_execution_id_high()
        let attachmentLow = seyal_bridge_attachment_id_low()
        let attachmentHigh = seyal_bridge_attachment_id_high()
        guard executionLow != 0 || executionHigh != 0 else { return }
        if lastBoundExecution == (executionLow, executionHigh) { return }

        let snapshot = seyal_app_snapshot(appHandle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = UInt16(SEYAL_APP_ACTION_BIND.rawValue)
        action.flags = UInt16(SEYAL_APP_FLAG_TARGET_CONTROLLER)
        if inputSurface.observedAlternateScreen {
            action.flags |= UInt16(SEYAL_APP_FLAG_ALTERNATE_SCREEN)
        }
        action.fence_pane_lo = snapshot.pane_lo
        action.fence_pane_hi = snapshot.pane_hi
        action.fence_epoch = snapshot.epoch
        action.target_execution_lo = executionLow
        action.target_execution_hi = executionHigh
        action.target_attachment_lo = attachmentLow
        action.target_attachment_hi = attachmentHigh
        action.target_pty_generation = 1
        guard seyal_app_apply(appHandle, &action) == 0 else { return }
        lastBoundExecution = (executionLow, executionHigh)
        announceAccessibility()
    }

    private func announceAccessibility() {
        let tree = seyal_app_accessibility(appHandle)
        guard tree.node_count > 0, let nodes = tree.nodes else { return }
        let node = nodes.pointee
        guard node.label_len > 0, let labelBytes = node.label else { return }
        let label = String(
            decoding: UnsafeBufferPointer(start: labelBytes, count: Int(node.label_len)),
            as: UTF8.self
        )
        SeyalAccessibilityAnnouncement.post(label, element: self)
    }
}

