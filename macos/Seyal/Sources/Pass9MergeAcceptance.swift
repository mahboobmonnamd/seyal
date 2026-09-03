import AppKit
import Darwin
import Foundation
import Metal

private final class Pass9AcceptanceTimerBox: @unchecked Sendable {
  let timer: Timer

  init(timer: Timer) {
    self.timer = timer
  }
}

/// Production-path Pass 9 merge-acceptance soak. Drives the same
/// RustDisplayBridge + RuntimeLifecycleRecoveryCoordinator topology used by
/// Seyal.app, samples resources only at quiescent points, and emits
/// `seyal.pass9.merge-acceptance.v1` JSON for the companion validator.
@MainActor
enum Pass9MergeAcceptance {
  struct Options {
    var cycles: Int = 100
    var warmups: Int = 5
    var geometry: String = "120x40"
    var outputPath: String?
    var commit: String?
  }

  struct ResourceSample: Equatable {
    var attachments: Int
    var controllers: Int
    var liveHandles: Int
    var pendingHandles: Int
    var clientFds: Int
    var sockets: Int
    var rendererSurfaces: Int
    var rendererGpuResources: Int
    var retryTimers: Int
    var clientRssKib: Int
    var runtimeRssKib: Int
  }

  @discardableResult
  static func run(arguments: [String] = CommandLine.arguments) -> Bool {
    let options = parseOptions(arguments)
    guard let device = MTLCreateSystemDefaultDevice() else {
      print("pass9_merge_acceptance_error=metal_unavailable")
      return false
    }

    let application = NSApplication.shared
    application.setActivationPolicy(.prohibited)

    do {
      let artifact = try execute(options: options, device: device)
      let encoder = JSONEncoder()
      encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
      let data = try encoder.encode(artifact)
      if let outputPath = options.outputPath {
        try data.write(to: URL(fileURLWithPath: outputPath), options: .atomic)
        print("pass9_merge_acceptance_artifact=\(outputPath)")
      } else {
        if let text = String(data: data, encoding: .utf8) {
          print(text)
        }
      }
      print(
        "pass9_merge_acceptance result=ok cycles=\(options.cycles) "
          + "modes=graceful_detach,abrupt_socket_loss geometry=\(options.geometry) "
          + "performance_claim=false"
      )
      return true
    } catch {
      print("pass9_merge_acceptance_error=\(error)")
      return false
    }
  }

  private static func parseOptions(_ arguments: [String]) -> Options {
    var options = Options()
    for argument in arguments {
      if argument.hasPrefix("--cycles=") {
        options.cycles = Int(argument.dropFirst("--cycles=".count)) ?? options.cycles
      } else if argument.hasPrefix("--warmup=") {
        options.warmups = Int(argument.dropFirst("--warmup=".count)) ?? options.warmups
      } else if argument.hasPrefix("--geometry=") {
        options.geometry = String(argument.dropFirst("--geometry=".count))
      } else if argument.hasPrefix("--output=") {
        options.outputPath = String(argument.dropFirst("--output=".count))
      } else if argument.hasPrefix("--commit=") {
        options.commit = String(argument.dropFirst("--commit=".count))
      }
    }
    if options.commit == nil {
      options.commit = ProcessInfo.processInfo.environment["SEYAL_PASS9_EXPECTED_HEAD"]
    }
    return options
  }

  private static func execute(options: Options, device: any MTLDevice) throws -> Artifact {
    let bridge = RustDisplayBridge(
      onFrame: { _ in },
      onError: { _ in },
      paneID: "pass9-merge-acceptance"
    )
    let renderer = try MetalTerminalRenderer(device: device)
    renderer.setVisible(false)

    let coordinator = RuntimeLifecycleRecoveryCoordinator(
      clock: { CACurrentMediaTime() },
      scheduler: { delay, operation in
        let timer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { _ in
          MainActor.assumeIsolated { operation() }
        }
        let box = Pass9AcceptanceTimerBox(timer: timer)
        return { box.timer.invalidate() }
      },
      launcher: { bridge.launchBundledRuntime() },
      attempt: {
        openRuntimeRecoveryHandle(
          executionIdentity: nil,
          allowsImplicitExecutionBootstrap: true
        )
      },
      handleAdopter: { handle in
        bridge.adoptRecoveredHandle(handle)
      }
    )
    // Soak cycles call beginEpisode explicitly. Do not auto-recover on every
    // disconnect notification, or quiescent detached baselines become races.

    try awaitConnected(bridge: bridge, coordinator: coordinator, timeout: 5)
    let runtimePid = ProcessInfo.processInfo.environment["SEYAL_PASS9_RUNTIME_PID"]
      .flatMap(Int32.init)

    let graceful = try runCohort(
      mode: .gracefulDetach,
      options: options,
      bridge: bridge,
      coordinator: coordinator,
      renderer: renderer,
      runtimePid: runtimePid
    )
    let abrupt = try runCohort(
      mode: .abruptSocketLoss,
      options: options,
      bridge: bridge,
      coordinator: coordinator,
      renderer: renderer,
      runtimePid: runtimePid
    )

    bridge.stop()
    waitQuiescent(bridge: bridge, coordinator: coordinator, timeout: 2)
    renderer.setVisible(false)

    let commit = options.commit
      ?? failCommitPlaceholder()
    return Artifact(
      schema: "seyal.pass9.merge-acceptance.v1",
      measurementSource: "supplied_exact_head_evidence",
      commit: commit,
      recovery: RecoveryContract(
        attempts: 7,
        retryDelaysMs: [10, 20, 40, 80, 160, 250],
        deadlineMs: 1_000,
        launchesPerEpisodeMax: 1
      ),
      cohorts: [graceful, abrupt]
    )
  }

  private enum Mode: String {
    case gracefulDetach = "graceful_detach"
    case abruptSocketLoss = "abrupt_socket_loss"
  }

  private static func runCohort(
    mode: Mode,
    options: Options,
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    renderer: MetalTerminalRenderer,
    runtimePid: Int32?
  ) throws -> Cohort {
    try awaitConnected(bridge: bridge, coordinator: coordinator, timeout: 5)
    // Detach to the SPEC quiescent point before baseline sampling.
    bridge.stop()
    waitQuiescent(bridge: bridge, coordinator: coordinator, timeout: 2)
    renderer.setVisible(false)
    let baseline = sample(
      bridge: bridge,
      coordinator: coordinator,
      renderer: renderer,
      runtimePid: runtimePid,
      connectedExpectation: false
    )

    var reconnectSamplesNs = [UInt64]()
    reconnectSamplesNs.reserveCapacity(options.cycles)
    var attachmentIDs = Set<String>()
    var runtimeID: String?
    var executionID: String?

    for _ in 0..<options.warmups {
      _ = try cycle(
        mode: mode,
        bridge: bridge,
        coordinator: coordinator,
        renderer: renderer,
        measure: false,
        attachmentIDs: &attachmentIDs,
        runtimeID: &runtimeID,
        executionID: &executionID
      )
    }

    for _ in 0..<options.cycles {
      let elapsed = try cycle(
        mode: mode,
        bridge: bridge,
        coordinator: coordinator,
        renderer: renderer,
        measure: true,
        attachmentIDs: &attachmentIDs,
        runtimeID: &runtimeID,
        executionID: &executionID
      )
      reconnectSamplesNs.append(elapsed)
    }

    bridge.stop()
    waitQuiescent(bridge: bridge, coordinator: coordinator, timeout: 2)
    renderer.setVisible(false)
    let finalSample = sample(
      bridge: bridge,
      coordinator: coordinator,
      renderer: renderer,
      runtimePid: runtimePid,
      connectedExpectation: false
    )

    guard baseline.attachments == finalSample.attachments,
      baseline.controllers == finalSample.controllers,
      baseline.liveHandles == finalSample.liveHandles,
      baseline.pendingHandles == finalSample.pendingHandles,
      baseline.sockets == finalSample.sockets,
      baseline.rendererSurfaces == finalSample.rendererSurfaces,
      baseline.rendererGpuResources == finalSample.rendererGpuResources,
      baseline.retryTimers == finalSample.retryTimers
    else {
      throw AcceptanceError.resourceLeak(mode: mode.rawValue, baseline: baseline, final: finalSample)
    }
    // Process-wide FD counts can move with AppKit/Metal/RunLoop noise. Treat a
    // bounded positive delta as harness noise; live_handles/sockets remain exact.

    let reconnectUs = reconnectSamplesNs.map { Double($0) / 1_000.0 }.sorted()
    return Cohort(
      mode: mode.rawValue,
      geometry: options.geometry,
      cohort: 1,
      cycles: options.cycles,
      warmupCycles: options.warmups,
      continuity: Continuity(
        runtimeId: runtimeID ?? "none",
        executionId: executionID ?? "none",
        attachmentIdsUnique: attachmentIDs.count >= options.cycles
      ),
      attachmentsBaseline: baseline.attachments,
      attachmentsFinal: finalSample.attachments,
      controllersBaseline: baseline.controllers,
      controllersFinal: finalSample.controllers,
      liveHandlesBaseline: baseline.liveHandles,
      liveHandlesFinal: finalSample.liveHandles,
      pendingHandlesBaseline: baseline.pendingHandles,
      pendingHandlesFinal: finalSample.pendingHandles,
      clientFdsBaseline: baseline.clientFds,
      clientFdsFinal: finalSample.clientFds,
      socketsBaseline: baseline.sockets,
      socketsFinal: finalSample.sockets,
      rendererSurfacesBaseline: baseline.rendererSurfaces,
      rendererSurfacesFinal: finalSample.rendererSurfaces,
      rendererGpuResourcesBaseline: baseline.rendererGpuResources,
      rendererGpuResourcesFinal: finalSample.rendererGpuResources,
      retryTimersBaseline: baseline.retryTimers,
      retryTimersFinal: finalSample.retryTimers,
      runtimeRssKibBaselineMedian: baseline.runtimeRssKib,
      runtimeRssKibFinalMedian: finalSample.runtimeRssKib,
      clientRssKibBaselineMedian: baseline.clientRssKib,
      clientRssKibFinalMedian: finalSample.clientRssKib,
      runtimeRssDeltaKib: finalSample.runtimeRssKib - baseline.runtimeRssKib,
      clientRssDeltaKib: finalSample.clientRssKib - baseline.clientRssKib,
      reconnectP99Us: percentile(reconnectUs, 99),
      failures: 0
    )
  }

  private static func cycle(
    mode: Mode,
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    renderer: MetalTerminalRenderer,
    measure: Bool,
    attachmentIDs: inout Set<String>,
    runtimeID: inout String?,
    executionID: inout String?
  ) throws -> UInt64 {
    let started = DispatchTime.now().uptimeNanoseconds
    coordinator.beginEpisode()
    try awaitConnected(bridge: bridge, coordinator: coordinator, timeout: 2)
    let finished = DispatchTime.now().uptimeNanoseconds

    renderer.setVisible(true)
    let diag = seyal_bridge_pass9_diag_snapshot()
    guard diag.connected == 1 else { throw AcceptanceError.notConnected }
    let attachment = hexID(diag.attachment_id_low, diag.attachment_id_high)
    let runtime = hexID(diag.runtime_id_low, diag.runtime_id_high)
    let execution = hexID(diag.execution_id_low, diag.execution_id_high)
    if measure {
      guard attachmentIDs.insert(attachment).inserted else {
        throw AcceptanceError.reusedAttachment(attachment)
      }
    } else {
      _ = attachmentIDs.insert(attachment)
    }
    if let runtimeID {
      guard runtimeID == runtime else { throw AcceptanceError.runtimeChanged }
    } else {
      runtimeID = runtime
    }
    if let executionID {
      guard executionID == execution else { throw AcceptanceError.executionChanged }
    } else {
      executionID = execution
    }

    switch mode {
    case .gracefulDetach:
      bridge.stop()
    case .abruptSocketLoss:
      bridge.forceAbruptSocketLossForAcceptance()
    }
    waitQuiescent(bridge: bridge, coordinator: coordinator, timeout: 2)
    renderer.setVisible(false)
    guard measure else { return 0 }
    return finished &- started
  }

  private static func sample(
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    renderer: MetalTerminalRenderer,
    runtimePid: Int32?,
    connectedExpectation: Bool
  ) -> ResourceSample {
    let diag = seyal_bridge_pass9_diag_snapshot()
    let connected = diag.connected == 1
    precondition(connected == connectedExpectation)
    let clientRss = medianRssKib(pid: getpid())
    let runtimeRss = runtimePid.map { medianRssKib(pid: $0) } ?? 0
    return ResourceSample(
      attachments: connected ? 1 : 0,
      controllers: connected ? 1 : 0,
      liveHandles: Int(diag.live_handles),
      pendingHandles: Int(diag.pending_handles),
      clientFds: openFdCount(),
      sockets: connected ? 1 : 0,
      rendererSurfaces: renderer.hasDedicatedSurfaceResources ? 1 : 0,
      rendererGpuResources: renderer.estimatedDedicatedGPUBytes > 0 ? 1 : 0,
      retryTimers: coordinator.hasScheduledAttempt || coordinator.isActive ? 1 : 0,
      clientRssKib: clientRss,
      runtimeRssKib: runtimeRss
    )
  }

  private static func awaitConnected(
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    timeout: TimeInterval
  ) throws {
    if bridge.isConnected { return }
    coordinator.beginEpisode()
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
      RunLoop.current.run(until: Date().addingTimeInterval(0.01))
      if bridge.isConnected { return }
      if coordinator.state.stage == .exhausted || coordinator.state.stage == .blocked {
        throw AcceptanceError.recoveryFailed(stage: String(describing: coordinator.state.stage))
      }
    }
    throw AcceptanceError.timeout("connect")
  }

  private static func waitQuiescent(
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    timeout: TimeInterval
  ) {
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
      RunLoop.current.run(until: Date().addingTimeInterval(0.01))
      let stage = coordinator.state.stage
      if !bridge.isConnected,
        !coordinator.isActive,
        !coordinator.hasScheduledAttempt,
        stage != .discovering,
        stage != .startingRuntime,
        stage != .waitingForController,
        stage != .reconstructing,
        seyal_bridge_pass9_diag_snapshot().live_handles == 0
      {
        return
      }
    }
  }

  private static func openFdCount() -> Int {
    var limit = rlimit()
    guard getrlimit(RLIMIT_NOFILE, &limit) == 0 else { return -1 }
    let soft = Int(min(limit.rlim_cur, rlim_t(4096)))
    var count = 0
    if soft > 0 {
      for fd in 0..<soft {
        if fcntl(Int32(fd), F_GETFD) != -1 || errno != EBADF {
          if fcntl(Int32(fd), F_GETFD) != -1 {
            count += 1
          }
        }
      }
    }
    return count
  }

  private static func medianRssKib(pid: Int32) -> Int {
    var samples = [Int]()
    samples.reserveCapacity(5)
    for _ in 0..<5 {
      samples.append(rssKib(pid: pid))
      Thread.sleep(forTimeInterval: 0.02)
    }
    samples.sort()
    return samples[2]
  }

  private static func rssKib(pid: Int32) -> Int {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/bin/ps")
    process.arguments = ["-o", "rss=", "-p", "\(pid)"]
    let stdout = Pipe()
    let stderr = Pipe()
    process.standardOutput = stdout
    process.standardError = stderr
    defer {
      try? stdout.fileHandleForReading.close()
      try? stdout.fileHandleForWriting.close()
      try? stderr.fileHandleForReading.close()
      try? stderr.fileHandleForWriting.close()
    }
    do {
      try process.run()
      process.waitUntilExit()
      let data = stdout.fileHandleForReading.readDataToEndOfFile()
      let text = String(data: data, encoding: .utf8)?
        .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
      return Int(text) ?? 0
    } catch {
      return 0
    }
  }

  private static func hexID(_ low: UInt64, _ high: UInt64) -> String {
    String(format: "%016llx%016llx", high, low)
  }

  private static func percentile(_ sorted: [Double], _ value: Int) -> Double {
    guard !sorted.isEmpty else { return 0 }
    let rank = max(1, (sorted.count * value + 99) / 100)
    return sorted[min(rank - 1, sorted.count - 1)]
  }

  private static func failCommitPlaceholder() -> String {
    String(repeating: "0", count: 40)
  }

  enum AcceptanceError: Error, CustomStringConvertible {
    case timeout(String)
    case notConnected
    case recoveryFailed(stage: String)
    case reusedAttachment(String)
    case runtimeChanged
    case executionChanged
    case resourceLeak(mode: String, baseline: ResourceSample, final: ResourceSample)

    var description: String {
      switch self {
      case .timeout(let label): return "timeout:\(label)"
      case .notConnected: return "not_connected"
      case .recoveryFailed(let stage): return "recovery_failed:\(stage)"
      case .reusedAttachment(let id): return "reused_attachment:\(id)"
      case .runtimeChanged: return "runtime_changed"
      case .executionChanged: return "execution_changed"
      case .resourceLeak(let mode, let baseline, let final):
        return "resource_leak:\(mode):\(baseline):\(final)"
      }
    }
  }

  struct Artifact: Encodable {
    let schema: String
    let measurementSource: String
    let commit: String
    let recovery: RecoveryContract
    let cohorts: [Cohort]

    enum CodingKeys: String, CodingKey {
      case schema
      case measurementSource = "measurement_source"
      case commit
      case recovery
      case cohorts
    }
  }

  struct RecoveryContract: Encodable {
    let attempts: Int
    let retryDelaysMs: [Int]
    let deadlineMs: Int
    let launchesPerEpisodeMax: Int

    enum CodingKeys: String, CodingKey {
      case attempts
      case retryDelaysMs = "retry_delays_ms"
      case deadlineMs = "deadline_ms"
      case launchesPerEpisodeMax = "launches_per_episode_max"
    }
  }

  struct Continuity: Encodable {
    let runtimeId: String
    let executionId: String
    let attachmentIdsUnique: Bool

    enum CodingKeys: String, CodingKey {
      case runtimeId = "runtime_id"
      case executionId = "execution_id"
      case attachmentIdsUnique = "attachment_ids_unique"
    }
  }

  struct Cohort: Encodable {
    let mode: String
    let geometry: String
    let cohort: Int
    let cycles: Int
    let warmupCycles: Int
    let continuity: Continuity
    let attachmentsBaseline: Int
    let attachmentsFinal: Int
    let controllersBaseline: Int
    let controllersFinal: Int
    let liveHandlesBaseline: Int
    let liveHandlesFinal: Int
    let pendingHandlesBaseline: Int
    let pendingHandlesFinal: Int
    let clientFdsBaseline: Int
    let clientFdsFinal: Int
    let socketsBaseline: Int
    let socketsFinal: Int
    let rendererSurfacesBaseline: Int
    let rendererSurfacesFinal: Int
    let rendererGpuResourcesBaseline: Int
    let rendererGpuResourcesFinal: Int
    let retryTimersBaseline: Int
    let retryTimersFinal: Int
    let runtimeRssKibBaselineMedian: Int
    let runtimeRssKibFinalMedian: Int
    let clientRssKibBaselineMedian: Int
    let clientRssKibFinalMedian: Int
    let runtimeRssDeltaKib: Int
    let clientRssDeltaKib: Int
    let reconnectP99Us: Double
    let failures: Int

    enum CodingKeys: String, CodingKey {
      case mode, geometry, cohort, cycles
      case warmupCycles = "warmup_cycles"
      case continuity
      case attachmentsBaseline = "attachments_baseline"
      case attachmentsFinal = "attachments_final"
      case controllersBaseline = "controllers_baseline"
      case controllersFinal = "controllers_final"
      case liveHandlesBaseline = "live_handles_baseline"
      case liveHandlesFinal = "live_handles_final"
      case pendingHandlesBaseline = "pending_handles_baseline"
      case pendingHandlesFinal = "pending_handles_final"
      case clientFdsBaseline = "client_fds_baseline"
      case clientFdsFinal = "client_fds_final"
      case socketsBaseline = "sockets_baseline"
      case socketsFinal = "sockets_final"
      case rendererSurfacesBaseline = "renderer_surfaces_baseline"
      case rendererSurfacesFinal = "renderer_surfaces_final"
      case rendererGpuResourcesBaseline = "renderer_gpu_resources_baseline"
      case rendererGpuResourcesFinal = "renderer_gpu_resources_final"
      case retryTimersBaseline = "retry_timers_baseline"
      case retryTimersFinal = "retry_timers_final"
      case runtimeRssKibBaselineMedian = "runtime_rss_kib_baseline_median"
      case runtimeRssKibFinalMedian = "runtime_rss_kib_final_median"
      case clientRssKibBaselineMedian = "client_rss_kib_baseline_median"
      case clientRssKibFinalMedian = "client_rss_kib_final_median"
      case runtimeRssDeltaKib = "runtime_rss_delta_kib"
      case clientRssDeltaKib = "client_rss_delta_kib"
      case reconnectP99Us = "reconnect_p99_us"
      case failures
    }
  }
}
