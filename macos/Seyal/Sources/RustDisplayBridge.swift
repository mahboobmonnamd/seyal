import Foundation

struct RuntimeBlockMetadata: Equatable, Sendable {
  enum State: UInt8, Sendable {
    case current = 1
    case completed = 2
  }

  let blockIDLow: UInt64
  let blockIDHigh: UInt64
  let revision: UInt64
  let startLineID: UInt64
  let state: State
}

/// UI registries must include the Pane namespace because Runtime block and
/// history request numbers are only unique within their owning execution.
struct PaneBlockKey: Hashable, Sendable {
  let paneID: String
  let blockID: UInt64

  init(paneID: String, blockID: UInt64) {
    self.paneID = paneID
    self.blockID = blockID
  }

  var accessibilityIdentifier: String {
    "pane.\(paneID).block.\(blockID)"
  }
}

struct PaneHistoryRequestKey: Hashable, Sendable {
  let paneID: String
  let requestID: UInt64

  init(paneID: String, requestID: UInt64) {
    self.paneID = paneID
    self.requestID = requestID
  }
}

struct NativeBlockRecord: Equatable {
  enum State: Equatable {
    case running
    case completed
  }

  let id: UInt64
  let command: String
  let state: State
  let startLine: UInt64
  let endLine: UInt64?
  let exitStatus: Int32
}

struct NativeHistoryRange: Equatable {
  struct Cell: Equatable {
    let scalar: UInt32
    let foreground: UInt32
    let background: UInt32
    let flags: UInt16
  }

  let startLine: UInt64
  let endLine: UInt64
  let blockID: UInt64
  let requestID: UInt64
  let revision: UInt64
  let rows: [[Cell]]
}

struct NativeComposerResult: Equatable {
  enum Code: Equatable {
    case accepted
    case busy
    case unsupported
    case backpressure
    case invalid
  }

  let requestID: UInt64
  let blockID: UInt64
  let code: Code
}

/// Geometry for one canonical Block projection on the Pane-owned Metal
/// surface. The surface consumes a complete frame, so lifecycle updates never
/// replace one history buffer at a time or expose a partially updated order.
struct NativeTranscriptRegion: Equatable {
  let id: UInt64
  let origin: NSPoint
  let clip: NSRect
}

struct NativeTranscriptFrame: Equatable {
  let revision: UInt64
  let regions: [NativeTranscriptRegion]
  let surfaceIdentity: ObjectIdentifier?

  init(
    revision: UInt64,
    regions: [NativeTranscriptRegion],
    surfaceIdentity: ObjectIdentifier? = nil
  ) {
    self.revision = revision
    self.regions = regions
    self.surfaceIdentity = surfaceIdentity
  }

  var regionIDs: [UInt64] { regions.map(\.id) }

  var isValid: Bool {
    guard regions.allSatisfy({ $0.id != 0 && $0.clip.width >= 0 && $0.clip.height >= 0 }) else {
      return false
    }
    return Set(regionIDs).count == regionIDs.count
  }

  func applyingDuplicateID(_ id: UInt64) -> NativeTranscriptFrame {
    NativeTranscriptFrame(
      revision: revision,
      regions: regions + [NativeTranscriptRegion(id: id, origin: .zero, clip: .zero)],
      surfaceIdentity: surfaceIdentity
    )
  }
}

/// Composer acceptance is correlated by the Runtime request ID. A command
/// string is intentionally not an identity: two successive submissions may be
/// identical and must still settle independently.
struct ComposerRequestCorrelation {
  private(set) var pendingRequestID: UInt64?
  private var nextRequestID: UInt64 = 1

  var isSettled: Bool { pendingRequestID == nil }

  mutating func begin(command: String) -> UInt64 {
    let requestID = nextRequestID
    nextRequestID = requestID == UInt64.max ? 1 : requestID + 1
    pendingRequestID = requestID
    _ = command
    return requestID
  }

  mutating func accepts(requestID: UInt64) -> Bool {
    guard let pendingRequestID, pendingRequestID == requestID
    else { return false }
    self.pendingRequestID = nil
    return true
  }
}

// Cancellation handlers may outlive RustDisplayBridge and deinit is
// nonisolated in Swift 6. The coordinator serializes its accounting and
// schedules the thread-local Rust disconnect on the main queue.
private final class RustBridgeHandleBox: @unchecked Sendable {
  var value: UInt64 = 0
}

private final class RustBridgeTeardownCoordinator: @unchecked Sendable {
  private let disconnect: () -> Void
  private let lock = NSLock()
  private var activeSourceCountStorage = 0
  private var disconnectPendingStorage = false
  private var disconnectScheduled = false
  var onDisconnected: (() -> Void)?

  var activeSourceCount: Int {
    lock.lock()
    defer { lock.unlock() }
    return activeSourceCountStorage
  }

  var disconnectPending: Bool {
    lock.lock()
    defer { lock.unlock() }
    return disconnectPendingStorage
  }

  init(disconnect: @escaping () -> Void) {
    self.disconnect = disconnect
  }

  func sourceCreated() {
    lock.lock()
    defer { lock.unlock() }
    activeSourceCountStorage += 1
  }

  func sourceCancelled() {
    lock.lock()
    guard activeSourceCountStorage > 0 else {
      lock.unlock()
      return
    }
    activeSourceCountStorage -= 1
    let shouldSchedule = disconnectPendingStorage && activeSourceCountStorage == 0
    lock.unlock()
    if shouldSchedule { scheduleDisconnectOnMain() }
  }

  func requestDisconnect() {
    lock.lock()
    guard !disconnectPendingStorage else {
      lock.unlock()
      return
    }
    disconnectPendingStorage = true
    let shouldSchedule = activeSourceCountStorage == 0
    lock.unlock()
    if shouldSchedule { scheduleDisconnectOnMain() }
  }

  private func scheduleDisconnectOnMain() {
    lock.lock()
    guard disconnectPendingStorage, activeSourceCountStorage == 0, !disconnectScheduled else {
      lock.unlock()
      return
    }
    disconnectScheduled = true
    lock.unlock()

    if Thread.isMainThread {
      finishDisconnectOnMain()
      return
    }

    DispatchQueue.main.async { [self] in
      finishDisconnectOnMain()
    }
  }

  private func finishDisconnectOnMain() {
    lock.lock()
    guard disconnectPendingStorage, activeSourceCountStorage == 0 else {
      disconnectScheduled = false
      lock.unlock()
      return
    }
    disconnectPendingStorage = false
    disconnectScheduled = false
    let callback = onDisconnected
    lock.unlock()

    // CLIENT is thread-local, so this must remain on the AppKit/main queue.
    dispatchPrecondition(condition: .onQueue(.main))
    disconnect()
    callback?()
  }
}

@MainActor
final class RustDisplayBridge {
  typealias FrameHandler = (SeyalPreparedFrame) -> Void
  typealias TimelineHandler = ([NativeBlockRecord]) -> Void
  typealias HistoryHandler = (NativeHistoryRange) -> Void
  typealias ComposerResultHandler = (NativeComposerResult) -> Void
  typealias ErrorHandler = (Int32) -> Void
  typealias StatusHandler = () -> Void

  struct RecoveryResult: Equatable {
    let stage: UInt8
    let failureClass: UInt8
    let retryable: Bool
    let connectionOrigin: UInt8
    let handle: UInt64
    let runtimeIDLow: UInt64
    let runtimeIDHigh: UInt64
    let executionIDLow: UInt64
    let executionIDHigh: UInt64
    let attachmentIDLow: UInt64
    let attachmentIDHigh: UInt64

    static func current() -> RecoveryResult {
      let result = seyal_bridge_last_recovery_result()
      return RecoveryResult(
        stage: result.stage,
        failureClass: result.failure_class,
        retryable: result.retryable != 0,
        connectionOrigin: result.connection_origin,
        handle: result.handle,
        runtimeIDLow: result.runtime_id_low,
        runtimeIDHigh: result.runtime_id_high,
        executionIDLow: result.execution_id_low,
        executionIDHigh: result.execution_id_high,
        attachmentIDLow: result.attachment_id_low,
        attachmentIDHigh: result.attachment_id_high
      )
    }
  }

  private let onFrame: FrameHandler
  private let onTimeline: TimelineHandler
  private let onHistory: HistoryHandler
  private let onComposerResult: ComposerResultHandler
  private let onError: ErrorHandler
  private let onStatusChanged: StatusHandler
  private var readSource: DispatchSourceRead?
  private var writeSource: DispatchSourceWrite?
  private var socketFileDescriptor: Int32 = -1
  private let handleBox = RustBridgeHandleBox()
  private var teardown: RustBridgeTeardownCoordinator!
  private(set) var clientHandle: UInt64 = 0
  private(set) var isConnected = false
  private(set) var lastRecoveryResult = RecoveryResult.current()
  /// The last bundled-helper failure is retained for the recovery coordinator
  /// to classify as blocked. Do not silently discard trust or spawn errors.
  private(set) var lastLaunchError: BundledRuntimeLaunchError?
  private(set) var runtimeIdentityWords: (low: UInt64, high: UInt64) = (0, 0)
  private(set) var attachmentIdentityWords: (low: UInt64, high: UInt64) = (0, 0)
  private(set) var reconstructionState = ReconnectReconstructionState()
  private(set) var runtimeBlockMetadata: RuntimeBlockMetadata?
  private var reconnectRequested = false
  private let runtimeLauncher = BundledRuntimeLauncher()
  private var lastTimelineRevision: UInt64 = 0
  private let paneID: String
  private let requestedExecutionIdentity: String?
  private let allowsImplicitExecutionBootstrap: Bool
  private var requestedHistoryRanges:
    [PaneHistoryRequestKey: (blockID: UInt64, startLine: UInt64, endLine: UInt64)] = [:]
  private var historyRevisions: [PaneHistoryRequestKey: (revision: UInt64, requestID: UInt64)] = [:]
  private var lastComposerResultRequestID: UInt64 = 0

  static func teardownReconnectStateSelfTest() -> Bool {
    var disconnects = 0
    let coordinator = RustBridgeTeardownCoordinator {
      disconnects += 1
    }
    coordinator.sourceCreated()
    coordinator.requestDisconnect()
    coordinator.requestDisconnect()
    guard coordinator.disconnectPending, disconnects == 0 else { return false }
    coordinator.sourceCancelled()
    let deadline = Date().addingTimeInterval(1)
    while disconnects == 0 && Date() < deadline {
      RunLoop.current.run(until: Date().addingTimeInterval(0.01))
    }
    return !coordinator.disconnectPending
      && coordinator.activeSourceCount == 0
      && disconnects == 1
  }

  init(
    onFrame: @escaping FrameHandler,
    onError: @escaping ErrorHandler,
    onStatusChanged: @escaping StatusHandler = {},
    onTimeline: @escaping TimelineHandler = { _ in },
    onHistory: @escaping HistoryHandler = { _ in },
    onComposerResult: @escaping ComposerResultHandler = { _ in },
    paneID: String = "unbound",
    executionIdentity: String? = nil,
    allowsImplicitExecutionBootstrap: Bool = true
  ) {
    self.onFrame = onFrame
    self.onTimeline = onTimeline
    self.onHistory = onHistory
    self.onComposerResult = onComposerResult
    self.onError = onError
    self.onStatusChanged = onStatusChanged
    self.paneID = paneID
    self.requestedExecutionIdentity = executionIdentity
    self.allowsImplicitExecutionBootstrap = allowsImplicitExecutionBootstrap
    teardown = RustBridgeTeardownCoordinator { [handleBox] in
      guard handleBox.value != 0 else { return }
      seyal_bridge_disconnect_handle(handleBox.value)
      handleBox.value = 0
    }
    teardown.onDisconnected = { [weak self] in
      self?.clientHandle = 0
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
    lastLaunchError = nil
    reconstructionState.beginAttempt()

    let handle: UInt64
    if let executionIdentity = requestedExecutionIdentity,
      let (low, high) = Self.executionWords(from: executionIdentity)
    {
      handle = seyal_bridge_open_execution(low, high)
    } else if let execution = reconstructionState.expectedExecution {
      handle = seyal_bridge_open_execution(execution.low, execution.high)
    } else if allowsImplicitExecutionBootstrap {
      handle = seyal_bridge_open_first()
    } else {
      // A split production Pane without a Runtime identity is not allowed to
      // attach to whichever execution happens to be first. This keeps pane
      // ownership explicit and makes the missing execution a visible bridge
      // failure until the Runtime supplies one.
      onError(-6)
      onStatusChanged()
      return false
    }
    guard handle != 0 else {
      lastRecoveryResult = RecoveryResult.current()
      onError(-6)
      onStatusChanged()
      return false
    }
    lastRecoveryResult = RecoveryResult.current()
    return adoptOpenedHandle(handle, recoveryResult: lastRecoveryResult)
  }

  /// Called only on the MainActor after the lifecycle executor has completed
  /// the disposable Rust connection. Adoption moves the client into this
  /// Pane's executor-local registry before AppKit registers socket sources.
  @discardableResult
  func adoptRecoveredHandle(_ handle: UInt64) -> Bool {
    guard !isConnected, !teardown.disconnectPending else {
      seyal_bridge_disconnect_handle(handle)
      return false
    }
    reconnectRequested = false
    lastLaunchError = nil
    reconstructionState.beginAttempt()
    guard seyal_bridge_adopt_handle(handle) == 0 else {
      seyal_bridge_disconnect_handle(handle)
      onError(-1)
      onStatusChanged()
      return false
    }
    lastRecoveryResult = RecoveryResult.current()
    return finishAdoptedHandle(handle, recoveryResult: lastRecoveryResult)
  }

  @discardableResult
  private func adoptOpenedHandle(_ handle: UInt64, recoveryResult: RecoveryResult) -> Bool {
    guard seyal_bridge_adopt_handle(handle) == 0 else {
      seyal_bridge_disconnect_handle(handle)
      onError(-1)
      onStatusChanged()
      return false
    }
    return finishAdoptedHandle(handle, recoveryResult: recoveryResult)
  }

  @discardableResult
  private func finishAdoptedHandle(_ handle: UInt64, recoveryResult: RecoveryResult) -> Bool {
    let runtime = RuntimeContinuityIdentity(
      low: recoveryResult.runtimeIDLow,
      high: recoveryResult.runtimeIDHigh
    )
    let execution = RuntimeContinuityIdentity(
      low: recoveryResult.executionIDLow,
      high: recoveryResult.executionIDHigh
    )
    let attachment = RuntimeContinuityIdentity(
      low: recoveryResult.attachmentIDLow,
      high: recoveryResult.attachmentIDHigh
    )
    // A Rust client handle is published only after finish_attach has validated
    // Controller authority and atomically committed the complete initial
    // snapshot. Identity drift or attachment reuse fails closed here before
    // AppKit can submit input or expose stale presentation.
    guard reconstructionState.commit(
      runtime: runtime,
      execution: execution,
      attachment: attachment,
      controllerAuthorityCommitted: true,
      authoritativeSnapshotCommitted: true
    ) else {
      seyal_bridge_disconnect_handle(handle)
      onError(-4)
      onStatusChanged()
      return false
    }
    clientHandle = handle
    handleBox.value = handle
    runtimeIdentityWords = (runtime.low, runtime.high)
    attachmentIdentityWords = (attachment.low, attachment.high)

    let fileDescriptor = seyal_bridge_socket_fd()
    guard fileDescriptor >= 0 else {
      seyal_bridge_disconnect_handle(handle)
      clientHandle = 0
      handleBox.value = 0
      onError(fileDescriptor)
      onStatusChanged()
      return false
    }

    socketFileDescriptor = fileDescriptor
    isConnected = true
    runtimeBlockMetadata = currentBlockMetadata()
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
    onStatusChanged()
    return true
  }

  /// Starts only the trusted helper packaged inside Seyal.app. Episode-level
  /// launch-once ownership belongs to RuntimeLifecycleRecoveryCoordinator.
  @discardableResult
  func launchBundledRuntime() -> Bool {
    let result = runtimeLauncher.launch()
    if case let .failure(error) = result {
      lastLaunchError = error
      onError(error.nativeCode)
      onStatusChanged()
      return false
    }
    lastLaunchError = nil
    return true
  }

  static func executionWords(from value: String) -> (UInt64, UInt64)? {
    let normalized = value.trimmingCharacters(in: .whitespacesAndNewlines)
      .lowercased()
      .replacingOccurrences(of: "0x", with: "")
    guard normalized.count == 32,
      let high = UInt64(normalized.prefix(16), radix: 16),
      let low = UInt64(normalized.suffix(16), radix: 16)
    else { return nil }
    return (low, high)
  }

  func stop(reconnect: Bool = false) {
    reconnectRequested = reconnect
    guard isConnected || socketFileDescriptor >= 0 else { return }
    guard !teardown.disconnectPending else { return }

    isConnected = false
    reconstructionState.disconnect()
    runtimeBlockMetadata = nil
    // All request/display correlations are connection-local. A reconnect
    // receives a fresh attachment and must never reuse pending history,
    // composer, timeline, or generation state from the dead socket.
    requestedHistoryRanges.removeAll(keepingCapacity: false)
    historyRevisions.removeAll(keepingCapacity: false)
    lastTimelineRevision = 0
    lastComposerResultRequestID = 0
    runtimeIdentityWords = (0, 0)
    attachmentIdentityWords = (0, 0)
    socketFileDescriptor = -1
    _ = seyal_bridge_select(clientHandle)
    teardown.requestDisconnect()

    if let readSource {
      self.readSource = nil
      readSource.cancel()
    }
    if let writeSource {
      self.writeSource = nil
      writeSource.cancel()
    }
    onStatusChanged()
  }

  private func teardownCompleted() {
    onStatusChanged()
    guard reconnectRequested else { return }
    reconnectRequested = false
    _ = start()
  }

  @discardableResult
  private func selectClient() -> Bool {
    guard clientHandle != 0 else { return false }
    return seyal_bridge_select(clientHandle) == 0
  }

  func currentFrame() -> SeyalPreparedFrame? {
    guard isConnected, reconstructionState.canMutate, selectClient() else { return nil }
    let frame = seyal_bridge_frame()
    guard frame.cells != nil, frame.cell_count > 0 else { return nil }
    return frame
  }

  func publishCurrentFrame() {
    guard let frame = currentFrame() else { return }
    onFrame(frame)
  }

  /// Minimal read-only Pass 8 presentation seam. The rich command transcript
  /// remains the independent Pass 7.1 timeline above.
  func currentBlockMetadata() -> RuntimeBlockMetadata? {
    guard isConnected, reconstructionState.canMutate, selectClient() else { return nil }
    let value = seyal_bridge_execution_block_metadata()
    guard value.revision > 0,
      value.start_line_id > 0,
      let state = RuntimeBlockMetadata.State(rawValue: value.state)
    else { return nil }
    return RuntimeBlockMetadata(
      blockIDLow: value.block_id_low,
      blockIDHigh: value.block_id_high,
      revision: value.revision,
      startLineID: value.start_line_id,
      state: state
    )
  }

  func currentTimeline() -> [NativeBlockRecord] {
    guard isConnected, reconstructionState.canMutate, selectClient() else { return [] }
    let count = Int(seyal_bridge_block_count())
    return (0..<count).compactMap { index in
      let record = seyal_bridge_block_record(UInt32(index))
      guard record.id != 0, record.command != nil else { return nil }
      let command =
        String(
          bytes: UnsafeBufferPointer(
            start: record.command,
            count: Int(record.command_len)
          ),
          encoding: .utf8
        ) ?? ""
      return NativeBlockRecord(
        id: record.id,
        command: command,
        state: record.state == 0 ? .running : .completed,
        startLine: record.start_line,
        endLine: record.end_line == 0 ? nil : record.end_line,
        exitStatus: record.exit_status
      )
    }
  }

  func nextComposerRequestID() -> UInt64 {
    guard isConnected, reconstructionState.canMutate, selectClient() else { return 0 }
    return seyal_bridge_next_composer_request_id()
  }

  private func publishComposerResult() {
    guard isConnected, reconstructionState.canMutate, selectClient() else { return }
    let result = seyal_bridge_composer_result()
    guard result.request_id != 0,
      result.request_id != lastComposerResultRequestID
    else { return }
    lastComposerResultRequestID = result.request_id
    let code: NativeComposerResult.Code
    switch result.code {
    case 0: code = .accepted
    case 1: code = .busy
    case 2: code = .unsupported
    case 3: code = .backpressure
    default: code = .invalid
    }
    onComposerResult(
      NativeComposerResult(
        requestID: result.request_id,
        blockID: result.block_id,
        code: code
      ))
  }

  @discardableResult
  func requestHistoryRange(startLine: UInt64, endLine: UInt64, blockID: UInt64) -> Int32 {
    guard isConnected, reconstructionState.canMutate, selectClient(),
      blockID > 0, startLine > 0, endLine >= startLine
    else { return -4 }
    let requestID = seyal_bridge_next_history_request_id()
    guard requestID != 0 else { return -4 }
    let result = finishMutation(
      seyal_bridge_request_history_range(blockID, startLine, endLine, 512, 131_072))
    if result == 0 {
      let requestKey = PaneHistoryRequestKey(paneID: paneID, requestID: requestID)
      requestedHistoryRanges[requestKey] = (blockID, startLine, endLine)
    }
    return result
  }

  func discardHistoryRequests(except blockIDs: Set<UInt64>) {
    requestedHistoryRanges = requestedHistoryRanges.filter { blockIDs.contains($0.value.blockID) }
    historyRevisions = historyRevisions.filter { requestKey, _ in
      requestedHistoryRanges[requestKey] != nil
    }
  }

  private func publishHistoryRanges() {
    guard isConnected, reconstructionState.canMutate, selectClient() else { return }
    for (requestKey, request) in Array(requestedHistoryRanges) {
      let metadata = seyal_bridge_history_range_peek_for(request.blockID, requestKey.requestID)
      guard metadata.block_id != 0,
        metadata.request_id != 0,
        metadata.block_id == request.blockID,
        metadata.request_id == requestKey.requestID,
        metadata.revision > 0,
        historyRevisions[requestKey]?.revision != metadata.revision
          || historyRevisions[requestKey]?.requestID != metadata.request_id
      else { continue }
      let rows = (0..<Int(metadata.row_count)).compactMap { index -> [NativeHistoryRange.Cell]? in
        let row = seyal_bridge_history_range_row_for(
          metadata.block_id,
          metadata.request_id,
          UInt32(index)
        )
        guard row.line_id != 0, let cells = row.cells else { return nil }
        return Array(UnsafeBufferPointer(start: cells, count: Int(row.cell_count))).map {
          NativeHistoryRange.Cell(
            scalar: $0.scalar,
            foreground: $0.foreground,
            background: $0.background,
            flags: $0.flags
          )
        }
      }
      historyRevisions[requestKey] = (metadata.revision, metadata.request_id)
      let nativeRange = NativeHistoryRange(
        startLine: metadata.start_line == 0 ? request.startLine : metadata.start_line,
        endLine: metadata.end_line == 0 ? request.endLine : metadata.end_line,
        blockID: metadata.block_id,
        requestID: metadata.request_id,
        revision: metadata.revision,
        rows: rows
      )
      onHistory(nativeRange)
      // The native handler receives copied rows and the complete typed
      // identity before this disposable Runtime cache entry is freed.
      _ = seyal_bridge_history_range_consume(metadata.block_id, metadata.request_id)
      requestedHistoryRanges.removeValue(forKey: requestKey)
      historyRevisions.removeValue(forKey: requestKey)
    }
  }

  @discardableResult
  func submitCommittedText(_ text: String) -> Int32 {
    guard isConnected, reconstructionState.canMutate, selectClient() else {
      onStatusChanged()
      return -10
    }
    let byteCount = text.utf8.count
    guard byteCount <= Int(UInt32.max) else {
      onStatusChanged()
      return -14
    }
    let count = UInt32(byteCount)
    let result =
      text.utf8.withContiguousStorageIfAvailable { buffer -> Int32 in
        seyal_bridge_submit_utf8(buffer.baseAddress, count)
      }
      ?? Array(text.utf8).withUnsafeBufferPointer { buffer in
        seyal_bridge_submit_utf8(buffer.baseAddress, count)
      }
    return finishMutation(result)
  }

  @discardableResult
  func submitComposerCommand(_ text: String) -> Int32 {
    guard isConnected, reconstructionState.canMutate, selectClient() else {
      onStatusChanged()
      return -10
    }
    let byteCount = text.utf8.count
    guard byteCount <= Int(UInt32.max) else {
      onStatusChanged()
      return -14
    }
    let count = UInt32(byteCount)
    let result =
      text.utf8.withContiguousStorageIfAvailable { buffer -> Int32 in
        seyal_bridge_submit_composer(buffer.baseAddress, count)
      }
      ?? Array(text.utf8).withUnsafeBufferPointer { buffer in
        seyal_bridge_submit_composer(buffer.baseAddress, count)
      }
    return finishMutation(result)
  }

  @discardableResult
  func submitKey(kind: UInt16, scalar: UInt32) -> Int32 {
    guard isConnected, reconstructionState.canMutate, selectClient() else {
      onStatusChanged()
      return -10
    }
    return finishMutation(seyal_bridge_submit_key(kind, scalar))
  }

  @discardableResult
  func proposeGeometry(
    viewportWidth: Double,
    viewportHeight: Double,
    horizontalInsets: Double,
    verticalInsets: Double,
    cellWidth: Double,
    cellHeight: Double,
    meaningfulLayoutEpoch: Bool
  ) -> Int32 {
    guard isConnected, reconstructionState.canMutate, selectClient() else {
      onStatusChanged()
      return -10
    }
    return finishMutation(
      seyal_bridge_propose_geometry(
        viewportWidth,
        viewportHeight,
        horizontalInsets,
        verticalInsets,
        cellWidth,
        cellHeight,
        meaningfulLayoutEpoch ? 1 : 0
      )
    )
  }

  @discardableResult
  func retryResize() -> Int32 {
    guard isConnected, reconstructionState.canMutate, selectClient() else {
      onStatusChanged()
      return -10
    }
    return finishMutation(seyal_bridge_retry_resize())
  }

  func inputFailureCode() -> Int32 {
    guard isConnected, reconstructionState.canMutate, selectClient() else { return 4 }
    return seyal_bridge_input_failure()
  }

  func resizeFailureCode() -> Int32 {
    guard isConnected, reconstructionState.canMutate, selectClient() else { return 201 }
    return seyal_bridge_resize_failure()
  }

  private func finishMutation(_ result: Int32) -> Int32 {
    synchronizeWriteReadinessSource()
    onStatusChanged()
    if result == -18 {
      onError(result)
      stop(reconnect: true)
    } else if result == -3 || result == -10 {
      onError(result)
      stop()
    }
    return result
  }

  private func drainReadyDisplayWork() {
    guard isConnected, selectClient() else { return }
    defer {
      synchronizeWriteReadinessSource()
      onStatusChanged()
    }

    // Rust bounds each poll by frame count and bytes. A small outer bound
    // prevents one high-volume terminal from monopolizing the AppKit queue;
    // unread socket data will immediately retrigger this dispatch source.
    for _ in 0..<8 {
      let result = seyal_bridge_poll()
      runtimeBlockMetadata = currentBlockMetadata()
      publishHistoryRanges()
      publishComposerResult()
      if result == 1 {
        publishCurrentFrame()
        let revision = seyal_bridge_block_timeline_revision()
        if revision != lastTimelineRevision {
          lastTimelineRevision = revision
          onTimeline(currentTimeline())
        }
        continue
      }
      if result == 0 {
        return
      }

      onError(result)
      stop(reconnect: result == -18)
      return
    }
  }

  private func synchronizeWriteReadinessSource() {
    guard isConnected, selectClient() else {
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
    guard isConnected, selectClient() else { return }
    let result = seyal_bridge_flush_writable()
    guard result == 0 else {
      onError(result)
      stop(reconnect: result == -18)
      return
    }
    synchronizeWriteReadinessSource()
    onStatusChanged()
  }
}
