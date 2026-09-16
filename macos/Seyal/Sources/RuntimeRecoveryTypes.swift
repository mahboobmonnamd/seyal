import Foundation

final class RuntimeRecoveryTimerBox: @unchecked Sendable {
  let timer: Timer

  init(timer: Timer) {
    self.timer = timer
  }
}

enum RuntimeRecoveryStage: UInt8, Equatable {
  case disconnected = 0
  case discovering = 1
  case startingRuntime = 2
  case waitingForController = 3
  case reconstructing = 4
  case restoringInteraction = 5
  case usable = 6
  case exhausted = 7
  case blocked = 8
}

struct RuntimeRecoveryState: Equatable {
  private(set) var stage: RuntimeRecoveryStage = .disconnected
  private(set) var generation: UInt64 = 0

  mutating func begin() {
    generation &+= 1
    stage = .discovering
  }

  mutating func transition(to next: RuntimeRecoveryStage) {
    stage = next
  }

  mutating func cancel() {
    generation &+= 1
    stage = .disconnected
  }

  mutating func retry() {
    begin()
  }
}
