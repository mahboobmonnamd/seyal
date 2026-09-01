import Foundation

enum RuntimeRecoveryAttemptOutcome: Equatable {
  case connected
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

  static let retryDelays: [TimeInterval] = [0.010, 0.020, 0.040, 0.080, 0.160, 0.250]
  static let maximumAttempts = retryDelays.count + 1
  static let episodeDeadline: TimeInterval = 1.0

  private let clock: Clock
  private let scheduler: Scheduler
  private let launcher: Launcher
  private let attempt: Attempt
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
  private(set) var attemptCount = 0
  private(set) var state = RuntimeRecoveryState()

  init(
    clock: @escaping Clock,
    scheduler: @escaping Scheduler,
    launcher: @escaping Launcher,
    attempt: @escaping Attempt
  ) {
    self.clock = clock
    self.scheduler = scheduler
    self.launcher = launcher
    self.attempt = attempt
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
    // those reads on the lifecycle queue so the AppKit actor never becomes
    // the blocking I/O executor. The coordinator remains serial: retry
    // callbacks cannot overtake an in-flight attempt.
    let outcome = lifecycleQueue.sync(execute: attempt)
    guard generation == state.generation else { return }
    switch outcome {
    case .connected:
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
