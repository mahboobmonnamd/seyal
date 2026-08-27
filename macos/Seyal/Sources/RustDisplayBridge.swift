import Foundation

// All production calls occur on the AppKit/main queue; the unchecked
// Sendable conformance is required only because Swift 6 deinit is
// nonisolated while it hands ownership to the cancellation handlers.
private final class RustBridgeTeardownCoordinator: @unchecked Sendable {
    private let disconnect: () -> Void
    private(set) var activeSourceCount = 0
    private(set) var disconnectPending = false
    var onDisconnected: (() -> Void)?

    init(disconnect: @escaping () -> Void = { seyal_bridge_disconnect() }) {
        self.disconnect = disconnect
    }

    func sourceCreated() {
        activeSourceCount += 1
    }

    func sourceCancelled() {
        guard activeSourceCount > 0 else { return }
        activeSourceCount -= 1
        if disconnectPending, activeSourceCount == 0 {
            finishDisconnect()
        }
    }

    func requestDisconnect() {
        guard !disconnectPending else { return }
        disconnectPending = true
        if activeSourceCount == 0 {
            finishDisconnect()
        }
    }

    private func finishDisconnect() {
        guard disconnectPending else { return }
        disconnectPending = false
        disconnect()
        onDisconnected?()
    }
}

@MainActor
final class RustDisplayBridge {
    typealias FrameHandler = (SeyalPreparedFrame) -> Void
    typealias ErrorHandler = (Int32) -> Void

    private let onFrame: FrameHandler
    private let onError: ErrorHandler
    private var readSource: DispatchSourceRead?
    private var writeSource: DispatchSourceWrite?
    private var socketFileDescriptor: Int32 = -1
    private let teardown = RustBridgeTeardownCoordinator()
    private(set) var isConnected = false
    private var reconnectRequested = false

    static func teardownReconnectStateSelfTest() -> Bool {
        var disconnects = 0
        let coordinator = RustBridgeTeardownCoordinator {
            disconnects += 1
        }
        coordinator.sourceCreated()
        coordinator.requestDisconnect()
        guard coordinator.disconnectPending, disconnects == 0 else { return false }
        coordinator.sourceCancelled()
        return !coordinator.disconnectPending
            && coordinator.activeSourceCount == 0
            && disconnects == 1
    }

    init(onFrame: @escaping FrameHandler, onError: @escaping ErrorHandler) {
        self.onFrame = onFrame
        self.onError = onError
        teardown.onDisconnected = { [weak self] in
            self?.teardownCompleted()
        }
    }

    @discardableResult
    func start() -> Bool {
        guard !isConnected else { return true }
        if teardown.disconnectPending {
            reconnectRequested = true
            return false
        }
        reconnectRequested = false

        let result = seyal_bridge_connect_first()
        guard result == 0 else {
            onError(result)
            return false
        }

        let fileDescriptor = seyal_bridge_socket_fd()
        guard fileDescriptor >= 0 else {
            seyal_bridge_disconnect()
            onError(fileDescriptor)
            return false
        }

        socketFileDescriptor = fileDescriptor
        isConnected = true
        let source = DispatchSource.makeReadSource(fileDescriptor: fileDescriptor, queue: .main)
        source.setEventHandler { [weak self] in
            self?.drainReadyDisplayWork()
        }
        source.setCancelHandler { [teardown] in
            teardown.sourceCancelled()
        }
        teardown.sourceCreated()
        readSource = source
        source.resume()

        publishCurrentFrame()
        synchronizeWriteReadinessSource()
        return true
    }

    func stop() {
        reconnectRequested = false
        guard isConnected || socketFileDescriptor >= 0 else { return }
        guard !teardown.disconnectPending else { return }

        isConnected = false
        teardown.requestDisconnect()

        if let readSource {
            self.readSource = nil
            readSource.cancel()
        }
        if let writeSource {
            self.writeSource = nil
            writeSource.cancel()
        }

    }

    private func teardownCompleted() {
        guard reconnectRequested else { return }
        reconnectRequested = false
        _ = start()
    }

    func publishCurrentFrame() {
        guard isConnected else { return }
        let frame = seyal_bridge_frame()
        guard frame.cells != nil, frame.cell_count > 0 else { return }
        onFrame(frame)
    }

    private func drainReadyDisplayWork() {
        guard isConnected else { return }
        defer { synchronizeWriteReadinessSource() }

        // Rust bounds each poll by frame count and bytes. A small outer bound
        // prevents one high-volume terminal from monopolizing the AppKit queue;
        // unread socket data will immediately retrigger this dispatch source.
        for _ in 0..<8 {
            let result = seyal_bridge_poll()
            if result == 1 {
                publishCurrentFrame()
                continue
            }
            if result == 0 {
                return
            }

            onError(result)
            stop()
            return
        }
    }

    private func synchronizeWriteReadinessSource() {
        guard isConnected else {
            writeSource?.cancel()
            writeSource = nil
            return
        }

        let wantsWrite = seyal_bridge_wants_write()
        if wantsWrite < 0 {
            onError(wantsWrite)
            stop()
            return
        }
        guard wantsWrite == 1 else {
            writeSource?.cancel()
            writeSource = nil
            return
        }
        guard writeSource == nil, socketFileDescriptor >= 0 else { return }

        let source = DispatchSource.makeWriteSource(
            fileDescriptor: socketFileDescriptor,
            queue: .main
        )
        source.setEventHandler { [weak self] in
            self?.flushReadyControlWork()
        }
        source.setCancelHandler { [teardown] in
            teardown.sourceCancelled()
        }
        teardown.sourceCreated()
        writeSource = source
        source.resume()
    }

    deinit {
        // The coordinator is retained by cancellation handlers, so teardown
        // completes even if the owning surface destroys this bridge first.
        reconnectRequested = false
        teardown.requestDisconnect()
        readSource?.cancel()
        writeSource?.cancel()
    }

    private func flushReadyControlWork() {
        guard isConnected else { return }
        let result = seyal_bridge_flush_writable()
        guard result == 0 else {
            onError(result)
            stop()
            return
        }
        synchronizeWriteReadinessSource()
    }
}
