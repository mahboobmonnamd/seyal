import Foundation

@MainActor
final class RustDisplayBridge {
    typealias FrameHandler = (SeyalPreparedFrame) -> Void
    typealias ErrorHandler = (Int32) -> Void

    private let onFrame: FrameHandler
    private let onError: ErrorHandler
    private var readSource: DispatchSourceRead?
    private var writeSource: DispatchSourceWrite?
    private var socketFileDescriptor: Int32 = -1
    private(set) var isConnected = false

    init(onFrame: @escaping FrameHandler, onError: @escaping ErrorHandler) {
        self.onFrame = onFrame
        self.onError = onError
    }

    @discardableResult
    func start() -> Bool {
        guard !isConnected else { return true }

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
        readSource = source
        source.resume()

        publishCurrentFrame()
        synchronizeWriteReadinessSource()
        return true
    }

    func stop() {
        readSource?.cancel()
        readSource = nil
        writeSource?.cancel()
        writeSource = nil
        socketFileDescriptor = -1
        if isConnected {
            seyal_bridge_disconnect()
            isConnected = false
        }
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
        writeSource = source
        source.resume()
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
