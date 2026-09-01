import Foundation

enum RuntimeRecoveryAttemptOutcome: Equatable {
  case connected
  case endpointMissing
  case retryable
  case controllerBusy
  case blocked
}

/// Deterministic owner of one SPEC-009 foreground recovery episode.
///
/// The coordinator owns retry/deadline/launch accounting only. Runtime, PTY,
/// attachment, and terminal-state authority remain behind the injected attempt
/// and launcher hooks.
@MainActor
final class RuntimeLifecycleRecoveryCoordinator {
  typealias Cancellation = () -> Void
  typealias Clock = () -> TimeInterval
  typealias ScheduledOperation = @MainActor @Sendable () -> Void
  typealias Scheduler = (TimeInterval, @escaping ScheduledOperation) -> Cancellation
  typealias Launcher = () -> Void
  typealias Attempt = () -> RuntimeRecoveryAttemptOutcome

  static let retryDelays: [TimeInterval] = [0.010, 0.020, 0.040, 0.080, 0.160, 0.250]
  static let maximumAttempts = retryDelays.count + 1
  static let episodeDeadline: TimeInterval = 1.0

  private let clock: Clock
  private let scheduler: Scheduler
  private let launcher: Launcher
  private let attempt: Attempt
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
    let outcome = attempt()
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
