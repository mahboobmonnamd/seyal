import Foundation

/// Lifecycle-queue-only FFI attempt. It owns no AppKit state and returns only
/// a pending Rust handle; MainActor later adopts that handle into its Pane.
func openRuntimeRecoveryHandle(
  executionIdentity: String?,
  allowsImplicitExecutionBootstrap: Bool
) -> RuntimeRecoveryAttemptOutcome {
  let handle: UInt64
  if let executionIdentity,
    let words = runtimeRecoveryExecutionWords(executionIdentity)
  {
    handle = seyal_bridge_open_execution(words.low, words.high)
  } else if allowsImplicitExecutionBootstrap {
    handle = seyal_bridge_open_first()
  } else {
    return .blocked
  }
  guard handle != 0 else {
    let result = seyal_bridge_last_recovery_result()
    if result.failure_class == 1, result.retryable != 0 { return .endpointMissing }
    if result.failure_class == 3, result.retryable != 0 { return .controllerBusy }
    return result.retryable != 0 ? .retryable : .blocked
  }
  return .opened(handle)
}

private func runtimeRecoveryExecutionWords(_ value: String) -> (low: UInt64, high: UInt64)? {
  let normalized = value.trimmingCharacters(in: .whitespacesAndNewlines)
    .lowercased()
    .replacingOccurrences(of: "0x", with: "")
  guard normalized.count == 32,
    let high = UInt64(normalized.prefix(16), radix: 16),
    let low = UInt64(normalized.suffix(16), radix: 16)
  else { return nil }
  return (low, high)
}

enum RuntimeRecoveryAttemptOutcome: Equatable {
  case connected
  case opened(UInt64)
  case endpointMissing
  case retryable
  case controllerBusy
  case blocked
}

struct RuntimeContinuityIdentity: Equatable {
  let low: UInt64
  let high: UInt64

  static let none = RuntimeContinuityIdentity(low: 0, high: 0)
  var isValid: Bool { self != .none }
}

enum ReconnectReconstructionStage: Equatable {
  case disconnected
  case awaitingAuthoritativeSnapshot
  case usable
  case blockedIdentityMismatch
}

/// Pins the Runtime/execution continuity claim while treating every attachment
/// and all client-side reconstruction state as disposable.
struct ReconnectReconstructionState: Equatable {
  private(set) var stage: ReconnectReconstructionStage = .disconnected
  private(set) var expectedRuntime: RuntimeContinuityIdentity?
  private(set) var expectedExecution: RuntimeContinuityIdentity?
  private(set) var lastAttachment: RuntimeContinuityIdentity?

  var canMutate: Bool { stage == .usable }

  mutating func beginAttempt() {
    stage = .awaitingAuthoritativeSnapshot
  }

  mutating func commit(
    runtime: RuntimeContinuityIdentity,
    execution: RuntimeContinuityIdentity,
    attachment: RuntimeContinuityIdentity,
    controllerAuthorityCommitted: Bool,
    authoritativeSnapshotCommitted: Bool
  ) -> Bool {
    guard runtime.isValid, execution.isValid, attachment.isValid,
      expectedRuntime.map({ $0 == runtime }) ?? true,
      expectedExecution.map({ $0 == execution }) ?? true,
      lastAttachment.map({ $0 != attachment }) ?? true
    else {
      stage = .blockedIdentityMismatch
      return false
    }
    guard controllerAuthorityCommitted, authoritativeSnapshotCommitted else {
      stage = .awaitingAuthoritativeSnapshot
      return false
    }

    expectedRuntime = runtime
    expectedExecution = execution
    lastAttachment = attachment
    stage = .usable
    return true
  }

  mutating func disconnect() {
    stage = .disconnected
  }
}

/// Deterministic owner of one SPEC-009 foreground recovery episode.
///
/// The coordinator owns retry/deadline/launch accounting only. Runtime, PTY,
/// attachment, and terminal-state authority remain behind the injected attempt
/// and launcher hooks.
final class RuntimeLifecycleRecoveryCoordinator: @unchecked Sendable {
  typealias Cancellation = @Sendable () -> Void
  typealias Clock = () -> TimeInterval
  /// Scheduler callbacks are deliberately not MainActor isolated. Recovery
  /// discovery is lifecycle work, not presentation work, and must remain
  /// serial even while AppKit is busy rendering a frame.
  typealias ScheduledOperation = @Sendable () -> Void
  typealias Scheduler = @Sendable (TimeInterval, @escaping ScheduledOperation) -> Cancellation
  typealias Launcher = () -> Void
  typealias Attempt = () -> RuntimeRecoveryAttemptOutcome
  typealias HandleAdopter = (UInt64) -> Bool

  /// Production attempts are always dispatched to the lifecycle queue.  The
  /// inline mode exists solely for deterministic state-machine tests; it is
  /// never selected by a production call site.
  enum AttemptExecution: Equatable {
    case lifecycleQueue
    case inline
  }

  static let retryDelays: [TimeInterval] = [0.010, 0.020, 0.040, 0.080, 0.160, 0.250]
  static let maximumAttempts = retryDelays.count + 1
  static let episodeDeadline: TimeInterval = 1.0

  private let clock: Clock
  private let scheduler: Scheduler
  private let launcher: Launcher
  private let attempt: Attempt
  private let handleAdopter: HandleAdopter
  private let attemptExecution: AttemptExecution
  /// A dedicated serial queue is the ownership boundary for discovery,
  /// hello, attach and snapshot attempts. The queue is intentionally created
  /// per coordinator so a torn-down pane cannot share lifecycle work with a
  /// replacement pane.
  private let lifecycleQueue = DispatchQueue(
    label: "com.seyal.runtime.lifecycle-recovery",
    qos: .userInitiated
  )
  private var cancelScheduled: Cancellation?
  private var deadline: TimeInterval?
  private var launchClaimed = false
  private var inFlightAttempt: DispatchWorkItem?
  private(set) var attemptCount = 0
  private(set) var state = RuntimeRecoveryState()

  init(
    clock: @escaping Clock,
    scheduler: @escaping Scheduler,
    launcher: @escaping Launcher,
    attempt: @escaping Attempt,
    handleAdopter: @escaping HandleAdopter = { _ in false },
    attemptExecution: AttemptExecution = .lifecycleQueue
  ) {
    self.clock = clock
    self.scheduler = scheduler
    self.launcher = launcher
    self.attempt = attempt
    self.handleAdopter = handleAdopter
    self.attemptExecution = attemptExecution
  }

  var hasScheduledAttempt: Bool { cancelScheduled != nil }

  var isActive: Bool {
    deadline != nil
  }

  /// Begins a new foreground episode and invalidates every callback from the
  /// prior generation before making the required immediate attempt at t=0.
  func beginEpisode() {
    replaceGeneration(stage: .discovering)
    deadline = clock() + Self.episodeDeadline
    launchClaimed = false
    attemptCount = 0
    performAttempt(generation: state.generation)
  }

  /// An explicit user retry is a distinct foreground episode with fresh
  /// launch and retry budgets.
  func retry() {
    beginEpisode()
  }

  func cancel() {
    inFlightAttempt?.cancel()
    inFlightAttempt = nil
    cancelScheduled?()
    cancelScheduled = nil
    deadline = nil
    launchClaimed = false
    attemptCount = 0
    state.cancel()
  }

  func transition(to stage: RuntimeRecoveryStage) {
    state.transition(to: stage)
  }

  private func replaceGeneration(stage: RuntimeRecoveryStage) {
    inFlightAttempt?.cancel()
    inFlightAttempt = nil
    cancelScheduled?()
    cancelScheduled = nil
    deadline = nil
    state.begin()
    state.transition(to: stage)
  }

  private func performAttempt(generation: UInt64) {
    guard generation == state.generation, let deadline else { return }
    cancelScheduled = nil
    guard clock() < deadline, attemptCount < Self.maximumAttempts else {
      exhaust(generation: generation)
      return
    }

    attemptCount += 1
    // Local IPC performs bounded blocking reads during hello/attach. Execute
    // those reads asynchronously on the lifecycle queue so the AppKit actor
    // never becomes the blocking I/O executor. Completion is marshalled back
    // to the caller's actor; stale generations and cancelled work are dropped.
    guard attemptExecution == .lifecycleQueue else {
      completeAttempt(attempt(), generation: generation)
      return
    }
    let work = DispatchWorkItem { [attempt] in
      let outcome = attempt()
      DispatchQueue.main.async { [weak self] in
        self?.completeAttempt(outcome, generation: generation)
      }
    }
    inFlightAttempt = work
    lifecycleQueue.async(execute: work)
  }

  private func completeAttempt(
    _ outcome: RuntimeRecoveryAttemptOutcome,
    generation: UInt64
  ) {
    inFlightAttempt = nil
    guard generation == state.generation else {
      discardPendingHandle(from: outcome)
      return
    }
    guard let deadline, clock() < deadline else {
      discardPendingHandle(from: outcome)
      exhaust(generation: generation)
      return
    }
    switch outcome {
    case .connected:
      finishConnected(generation: generation)
    case let .opened(handle):
      guard handleAdopter(handle) else {
        seyal_bridge_disconnect_handle(handle)
        state.transition(to: .blocked)
        self.deadline = nil
        return
      }
      finishConnected(generation: generation)
    case .endpointMissing:
      state.transition(to: .startingRuntime)
      if !launchClaimed {
        launchClaimed = true
        launcher()
      }
      scheduleRetry(generation: generation)
    case .controllerBusy:
      state.transition(to: .waitingForController)
      scheduleRetry(generation: generation)
    case .retryable:
      state.transition(to: .discovering)
      scheduleRetry(generation: generation)
    case .blocked:
      cancelScheduled?()
      cancelScheduled = nil
      self.deadline = nil
      state.transition(to: .blocked)
    }
  }

  private func discardPendingHandle(from outcome: RuntimeRecoveryAttemptOutcome) {
    if case let .opened(handle) = outcome {
      seyal_bridge_disconnect_handle(handle)
    }
  }

  private func scheduleRetry(generation: UInt64) {
    guard generation == state.generation, let deadline else { return }
    let retryIndex = attemptCount - 1
    guard retryIndex < Self.retryDelays.count else {
      exhaust(generation: generation)
      return
    }

    let delay = Self.retryDelays[retryIndex]
    guard clock() + delay < deadline else {
      exhaust(generation: generation)
      return
    }
    cancelScheduled = scheduler(delay) { [weak self] in
      self?.performAttempt(generation: generation)
    }
  }

  private func finishConnected(generation: UInt64) {
    guard generation == state.generation else { return }
    cancelScheduled?()
    cancelScheduled = nil
    deadline = nil
    state.transition(to: .reconstructing)
  }

  private func exhaust(generation: UInt64) {
    guard generation == state.generation else { return }
    cancelScheduled?()
    cancelScheduled = nil
    deadline = nil
    state.transition(to: .exhausted)
  }
}
