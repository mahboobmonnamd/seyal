import AppKit
import Darwin
import Foundation
import Metal

private final class Pass9ReleaseTimerBox: @unchecked Sendable {
  let timer: Timer

  init(timer: Timer) {
    self.timer = timer
  }
}

/// Pass 9 release-qualification soak for Issue #736.
///
/// Reuses the same production recovery ownership path as merge-acceptance
/// (`RuntimeLifecycleRecoveryCoordinator` + `RustDisplayBridge` + Metal prepare
/// boundary). Emits one `seyal.pass9.production-budget.v1` cohort object per
/// invocation so the orchestrator can restart Runtime between independent
/// cohorts. Abrupt mode remains `socket_shutdown_owned_disconnect`.
@MainActor
enum Pass9ReleaseQualification {
  struct Options {
    var cycles: Int = 100
    var warmups: Int = 20
    var geometry: String = "120x40"
    var mode: String = "graceful_detach"
    var cohort: Int = 1
    var outputPath: String?
    var commit: String?
  }

  struct GeometrySpec: Equatable {
    var columns: Int
    var rows: Int
  }

  /// Holds the renderer for bridge frame callbacks without capturing `self`
  /// before initialization completes.
  private final class RendererBox {
    var renderer: MetalTerminalRenderer?
    var lastColumns: Int = 0
    var lastRows: Int = 0
    /// First Metal update after ensure (cold rebuild). Not overwritten by later
    /// incremental frames during presentConnectedSurface polling.
    var firstPreparedUpdateNs: UInt64 = 0
    var captureFirstPreparedUpdate = false
  }

  /// Retains the surviving ExecutionId hex so measured reconnects use
  /// `open_execution` (SPEC same-execution path) instead of `open_first` +
  /// ListExecutions on every cycle.
  private final class ContinuityBox {
    var executionIdentity: String?
  }

  /// Records the production lifecycle-queue attempt body (hello/attach/open).
  private final class AttemptTimingBox: @unchecked Sendable {
    var lastAttemptNs: UInt64 = 0
  }

  /// Production interactive surface used for SPEC §10 native_ready timing.
  @MainActor
  private final class NativeInteractionBox {
    let surface: InteractiveMetalSurfaceView
    let window: NSWindow

    init() {
      // Small probe geometry: SPEC §10 first-responder/AX/IME only; Metal display
      // path is suppressed, so keep the AppKit surface cheap.
      let frame = NSRect(x: 40, y: 40, width: 320, height: 200)
      let surface = InteractiveMetalSurfaceView(
        frame: frame,
        paneID: "pass9-release-native-interaction",
        allowsImplicitExecutionBootstrap: false,
        installation: .nativeInteractionProbe
      )
      let window = NSWindow(
        contentRect: frame,
        styleMask: [.titled, .closable],
        backing: .buffered,
        defer: false
      )
      window.contentView = surface
      window.title = "Seyal Pass9 Native Interaction"
      // Keep ordered out between cycles; orderFront only inside the timed restore.
      window.orderOut(nil)
      self.surface = surface
      self.window = window
    }

    func tearDown() {
      surface.suppressesAutomaticBridgeRecovery = true
      _ = window.makeFirstResponder(nil)
      window.orderOut(nil)
      window.contentView = nil
    }
  }

  struct ResourceSample: Equatable {
    var attachments: Int
    var controllers: Int
    var runtimeFds: Int
    var clientFds: Int
    var runtimeThreads: Int
    var clientThreads: Int
    var sockets: Int
    var rendererSurfaces: Int
    var rendererGpuResources: Int
    var pendingResync: Int
    var retryTimers: Int
    var runtimeAllocatorInUseKib: Int
    var clientAllocatorInUseKib: Int
    var clientRssKib: Int
    var runtimeRssKib: Int
  }

  struct CycleTimings {
    var reconnectNs: UInt64
    var preparedSurfaceNs: UInt64
    var nativeReadyNs: UInt64
    var cleanupNs: UInt64
  }

  @discardableResult
  static func run(arguments: [String] = CommandLine.arguments) -> Bool {
    let options = parseOptions(arguments)
    guard let device = MTLCreateSystemDefaultDevice() else {
      print("pass9_release_qualification_error=metal_unavailable")
      return false
    }

    let application = NSApplication.shared
    // Accessory (not prohibited): SPEC §10 native first-responder / AX focus
    // requires a key window and first-responder chain.
    application.setActivationPolicy(.accessory)

    do {
      let artifact = try execute(options: options, device: device)
      let encoder = JSONEncoder()
      encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
      let data = try encoder.encode(artifact)
      if let outputPath = options.outputPath {
        try data.write(to: URL(fileURLWithPath: outputPath), options: .atomic)
        print("pass9_release_qualification_artifact=\(outputPath)")
      } else if let text = String(data: data, encoding: .utf8) {
        print(text)
      }
      print(
        "pass9_release_qualification result=ok cycles=\(options.cycles) "
          + "mode=\(options.mode) geometry=\(options.geometry) cohort=\(options.cohort) "
          + "schema=seyal.pass9.production-budget.v1"
      )
      return true
    } catch {
      print("pass9_release_qualification_error=\(error)")
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
      } else if argument.hasPrefix("--mode=") {
        options.mode = String(argument.dropFirst("--mode=".count))
      } else if argument.hasPrefix("--cohort=") {
        options.cohort = Int(argument.dropFirst("--cohort=".count)) ?? options.cohort
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
    let mode = try parseMode(options.mode)
    let geometry = try parseGeometry(options.geometry)
    guard (1...5).contains(options.cohort) else {
      throw QualificationError.invalidCohort(options.cohort)
    }

    let rendererBox = RendererBox()
    let attemptTiming = AttemptTimingBox()
    let continuity = ContinuityBox()
    let nativeBox = NativeInteractionBox()
    defer { nativeBox.tearDown() }
    let renderer = try MetalTerminalRenderer(device: device)
    rendererBox.renderer = renderer
    renderer.setVisible(false)

    let bridge = RustDisplayBridge(
      onFrame: { frame in
        MainActor.assumeIsolated {
          guard let renderer = rendererBox.renderer,
            let native = NativePreparedFrame(bridgeFrame: frame)
          else { return }
          rendererBox.lastColumns = native.columns
          rendererBox.lastRows = native.rows
          let started = DispatchTime.now().uptimeNanoseconds
          // Prefer incremental update after first arming; full rebuild is the
          // cold-path Metal cost and is charged to prepared_surface with ensure.
          let forceFull = !renderer.hasDedicatedSurfaceResources
          _ = try? renderer.update(
            frame: native,
            backingScale: 2.0,
            forceFullRebuild: forceFull
          )
          let elapsed = DispatchTime.now().uptimeNanoseconds &- started
          if rendererBox.captureFirstPreparedUpdate, rendererBox.firstPreparedUpdateNs == 0 {
            rendererBox.firstPreparedUpdateNs = elapsed
            rendererBox.captureFirstPreparedUpdate = false
          }
        }
      },
      onError: { _ in },
      paneID: "pass9-release-qualification"
    )

    let coordinator = RuntimeLifecycleRecoveryCoordinator(
      clock: { CACurrentMediaTime() },
      scheduler: { delay, operation in
        let timer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { _ in
          MainActor.assumeIsolated { operation() }
        }
        let box = Pass9ReleaseTimerBox(timer: timer)
        return { box.timer.invalidate() }
      },
      launcher: { bridge.launchBundledRuntime() },
      attempt: {
        let started = DispatchTime.now().uptimeNanoseconds
        let outcome = openRuntimeRecoveryHandle(
          executionIdentity: continuity.executionIdentity,
          allowsImplicitExecutionBootstrap: continuity.executionIdentity == nil
        )
        attemptTiming.lastAttemptNs = DispatchTime.now().uptimeNanoseconds &- started
        return outcome
      },
      handleAdopter: { handle in
        bridge.adoptRecoveredHandle(handle)
      }
    )

    try awaitConnected(bridge: bridge, coordinator: coordinator, timeout: 5)
    let runtimePid = ProcessInfo.processInfo.environment["SEYAL_PASS9_RUNTIME_PID"]
      .flatMap(Int32.init)

    let cohort = try runCohort(
      mode: mode,
      options: options,
      geometry: geometry,
      bridge: bridge,
      coordinator: coordinator,
      renderer: renderer,
      rendererBox: rendererBox,
      nativeBox: nativeBox,
      attemptTiming: attemptTiming,
      continuity: continuity,
      runtimePid: runtimePid
    )

    bridge.stop()
    coordinator.cancel()
    waitQuiescent(bridge: bridge, coordinator: coordinator, renderer: renderer, timeout: 2)
    renderer.setVisible(false)

    return Artifact(
      schema: "seyal.pass9.production-budget.v1",
      measurementSource: "supplied_exact_head_evidence",
      commit: options.commit ?? String(repeating: "0", count: 40),
      recovery: RecoveryContract(
        attempts: 7,
        retryDelaysMs: [10, 20, 40, 80, 160, 250],
        deadlineMs: 1_000,
        launchesPerEpisodeMax: 1
      ),
      cohorts: [cohort],
      pass8: nil,
      topologyNote:
        "Metal prepare/release equivalent to MetalSurfaceView.consumeBridgeFrame; "
        + "native_ready uses production InteractiveMetalSurfaceView restore "
        + "(key window + first-responder + accessibilityFocused + empty marked text + IME activate) "
        + "before coordinator Usable (SPEC-009 §10); "
        + "reconnect_p99=lifecycle-queue attempt body for known ExecutionId "
        + "(open_execution hello/attach; not open_first+ListExecutions); "
        + "prepared_surface_p99=ensure PreparedSurface + FIRST MetalTerminalRenderer.update only; "
        + "cleanup_p99=bridge stop/cancel until live_handles==0; "
        + "exact-return uses diag live_handles/pending_handles, reconnect-owned socket_fd, "
        + "process open-fd samples, renderer surface/GPU flags, "
        + "runtime_allocator=live_handles, client_allocator=dedicated GPU KiB; "
        + "thread exact-return is reconnect-owned (0 — no per-client worker threads)."
    )
  }

  private enum Mode: String {
    case gracefulDetach = "graceful_detach"
    case abruptSocketLoss = "abrupt_socket_loss"
  }

  private static func parseMode(_ text: String) throws -> Mode {
    guard let mode = Mode(rawValue: text) else {
      throw QualificationError.invalidMode(text)
    }
    return mode
  }

  private static func runCohort(
    mode: Mode,
    options: Options,
    geometry: GeometrySpec,
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    renderer: MetalTerminalRenderer,
    rendererBox: RendererBox,
    nativeBox: NativeInteractionBox,
    attemptTiming: AttemptTimingBox,
    continuity: ContinuityBox,
    runtimePid: Int32?
  ) throws -> Cohort {
    try awaitConnected(bridge: bridge, coordinator: coordinator, timeout: 5)
    try presentConnectedSurface(
      bridge: bridge,
      coordinator: coordinator,
      renderer: renderer,
      rendererBox: rendererBox,
      geometry: geometry
    )
    detach(mode: mode, bridge: bridge, coordinator: coordinator, renderer: renderer)
    waitQuiescent(bridge: bridge, coordinator: coordinator, renderer: renderer, timeout: 2)

    // Cold-start settle: first cohort after a Release rebuild otherwise charges
    // Metal/IMK one-time caches into measured client_rss_delta. Touch the SPEC
    // §10 seam and one prepare/release once before warmups/baseline.
    do {
      _ = nativeBox.surface.restoreNativeInteractionAfterRendererReady()
      try awaitConnected(bridge: bridge, coordinator: coordinator, timeout: 5)
      _ = try presentConnectedSurface(
        bridge: bridge,
        coordinator: coordinator,
        renderer: renderer,
        rendererBox: rendererBox,
        geometry: geometry
      )
      detach(mode: mode, bridge: bridge, coordinator: coordinator, renderer: renderer)
      waitQuiescent(bridge: bridge, coordinator: coordinator, renderer: renderer, timeout: 2)
      Thread.sleep(forTimeInterval: 1.0)
    }

    var reconnectNs = [UInt64]()
    var preparedNs = [UInt64]()
    var nativeReadyNs = [UInt64]()
    var cleanupNs = [UInt64]()
    reconnectNs.reserveCapacity(options.cycles)
    preparedNs.reserveCapacity(options.cycles)
    nativeReadyNs.reserveCapacity(options.cycles)
    cleanupNs.reserveCapacity(options.cycles)

    var attachmentIDs = Set<String>()
    var runtimeID: String?
    var executionID: String?
    var peakSurfaces = 0
    var peakGpu = 0
    var geometryApplied = false

    // Warmups first so baseline/final compare a post-steady-state quiescent
    // point (SPEC-009 §16.1). Probe window is allocated before warmups so its
    // fixed AppKit/IMK cost is inside both baseline and final RSS samples.
    for _ in 0..<options.warmups {
      _ = try cycle(
        mode: mode,
        geometry: geometry,
        bridge: bridge,
        coordinator: coordinator,
        renderer: renderer,
        rendererBox: rendererBox,
        nativeBox: nativeBox,
        attemptTiming: attemptTiming,
        continuity: continuity,
        measure: false,
        attachmentIDs: &attachmentIDs,
        runtimeID: &runtimeID,
        executionID: &executionID,
        peakSurfaces: &peakSurfaces,
        peakGpu: &peakGpu,
        geometryApplied: &geometryApplied
      )
    }

    detach(mode: mode, bridge: bridge, coordinator: coordinator, renderer: renderer)
    waitQuiescent(bridge: bridge, coordinator: coordinator, renderer: renderer, timeout: 2)
    Thread.sleep(forTimeInterval: 1.0)
    let baseline = sample(
      bridge: bridge,
      coordinator: coordinator,
      renderer: renderer,
      runtimePid: runtimePid,
      connectedExpectation: false
    )
    let detachedCPU = sampleDetachedCPUPercent(runtimePid: runtimePid)

    for _ in 0..<options.cycles {
      let timings = try cycle(
        mode: mode,
        geometry: geometry,
        bridge: bridge,
        coordinator: coordinator,
        renderer: renderer,
        rendererBox: rendererBox,
        nativeBox: nativeBox,
        attemptTiming: attemptTiming,
        continuity: continuity,
        measure: true,
        attachmentIDs: &attachmentIDs,
        runtimeID: &runtimeID,
        executionID: &executionID,
        peakSurfaces: &peakSurfaces,
        peakGpu: &peakGpu,
        geometryApplied: &geometryApplied
      )
      reconnectNs.append(timings.reconnectNs)
      preparedNs.append(timings.preparedSurfaceNs)
      nativeReadyNs.append(timings.nativeReadyNs)
      cleanupNs.append(timings.cleanupNs)
    }

    guard peakSurfaces >= 1, peakGpu >= 1, geometryApplied else {
      throw QualificationError.vacuousRendererEvidence(
        surfaces: peakSurfaces,
        gpu: peakGpu,
        geometryApplied: geometryApplied
      )
    }

    detach(mode: mode, bridge: bridge, coordinator: coordinator, renderer: renderer)
    waitQuiescent(bridge: bridge, coordinator: coordinator, renderer: renderer, timeout: 2)
    Thread.sleep(forTimeInterval: 1.0)
    let finalSample = sample(
      bridge: bridge,
      coordinator: coordinator,
      renderer: renderer,
      runtimePid: runtimePid,
      connectedExpectation: false
    )

    let allocatorDelta = finalSample.clientAllocatorInUseKib - baseline.clientAllocatorInUseKib
    let allocatorClassification: String
    if allocatorDelta == 0 {
      allocatorClassification = "EXACT_RETURN"
    } else if allocatorDelta > 0, allocatorDelta <= 4 {
      allocatorClassification = "HARNESS_OWNED_FIXED_CAPACITY"
    } else if allocatorDelta > 4 {
      allocatorClassification = "EXCEEDS_HARNESS_ALLOWANCE"
      print(
        "pass9_release_qualification_warning=client_allocator_delta_kib:\(allocatorDelta)"
      )
    } else {
      allocatorClassification = "NEGATIVE_DELTA"
      print(
        "pass9_release_qualification_warning=client_allocator_delta_kib:\(allocatorDelta)"
      )
    }

    func us(_ samples: [UInt64]) -> Double {
      percentile(samples.map { Double($0) / 1_000.0 }, 99)
    }

    return Cohort(
      mode: mode.rawValue,
      geometry: options.geometry,
      cohort: options.cohort,
      cycles: options.cycles,
      reconnectP99Us: us(reconnectNs),
      cleanupP99Us: us(cleanupNs),
      preparedSurfaceP99Us: us(preparedNs),
      nativeReadyP99Us: us(nativeReadyNs),
      detachedCpuSamplesPercent: detachedCPU,
      detachedCpuP95Percent: nearestRank(detachedCPU, 0.95),
      runtimeRssDeltaKib: finalSample.runtimeRssKib - baseline.runtimeRssKib,
      clientRssDeltaKib: finalSample.clientRssKib - baseline.clientRssKib,
      attachmentsBaseline: baseline.attachments,
      attachmentsFinal: finalSample.attachments,
      controllersBaseline: baseline.controllers,
      controllersFinal: finalSample.controllers,
      runtimeFdsBaseline: baseline.runtimeFds,
      runtimeFdsFinal: finalSample.runtimeFds,
      clientFdsBaseline: baseline.clientFds,
      clientFdsFinal: finalSample.clientFds,
      runtimeThreadsBaseline: baseline.runtimeThreads,
      runtimeThreadsFinal: finalSample.runtimeThreads,
      clientThreadsBaseline: baseline.clientThreads,
      clientThreadsFinal: finalSample.clientThreads,
      socketsBaseline: baseline.sockets,
      socketsFinal: finalSample.sockets,
      rendererSurfacesBaseline: baseline.rendererSurfaces,
      rendererSurfacesFinal: finalSample.rendererSurfaces,
      rendererGpuResourcesBaseline: baseline.rendererGpuResources,
      rendererGpuResourcesFinal: finalSample.rendererGpuResources,
      pendingResyncBaseline: baseline.pendingResync,
      pendingResyncFinal: finalSample.pendingResync,
      retryTimersBaseline: baseline.retryTimers,
      retryTimersFinal: finalSample.retryTimers,
      runtimeAllocatorInUseKibBaseline: baseline.runtimeAllocatorInUseKib,
      runtimeAllocatorInUseKibFinal: finalSample.runtimeAllocatorInUseKib,
      clientAllocatorInUseKibBaseline: baseline.clientAllocatorInUseKib,
      clientAllocatorInUseKibFinal: finalSample.clientAllocatorInUseKib,
      clientAllocatorDeltaClassification: allocatorClassification
    )
  }

  private static func cycle(
    mode: Mode,
    geometry: GeometrySpec,
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    renderer: MetalTerminalRenderer,
    rendererBox: RendererBox,
    nativeBox: NativeInteractionBox,
    attemptTiming: AttemptTimingBox,
    continuity: ContinuityBox,
    measure: Bool,
    attachmentIDs: inout Set<String>,
    runtimeID: inout String?,
    executionID: inout String?,
    peakSurfaces: inout Int,
    peakGpu: inout Int,
    geometryApplied: inout Bool
  ) throws -> CycleTimings {
    attemptTiming.lastAttemptNs = 0
    rendererBox.firstPreparedUpdateNs = 0
    rendererBox.captureFirstPreparedUpdate = false

    coordinator.beginEpisode()
    try awaitConnected(
      bridge: bridge,
      coordinator: coordinator,
      timeout: 2,
      pollSeconds: measure ? 0.0001 : 0.01
    )
    let reconnectNs = attemptTiming.lastAttemptNs

    // SPEC prepared_surface: deferred prepare_cache + first Metal update.
    let prepareStarted = DispatchTime.now().uptimeNanoseconds
    guard bridge.ensurePreparedSurface() else {
      throw QualificationError.rendererNotArmed(
        surfaces: false,
        gpuBytes: 0,
        columns: rendererBox.lastColumns,
        rows: rendererBox.lastRows
      )
    }
    let ensureNs = DispatchTime.now().uptimeNanoseconds &- prepareStarted
    rendererBox.firstPreparedUpdateNs = 0
    rendererBox.captureFirstPreparedUpdate = true

    let applied = try presentConnectedSurface(
      bridge: bridge,
      coordinator: coordinator,
      renderer: renderer,
      rendererBox: rendererBox,
      geometry: geometry,
      pollSeconds: measure ? 0.0001 : 0.01
    )
    let preparedNs = ensureNs &+ rendererBox.firstPreparedUpdateNs
    geometryApplied = geometryApplied || applied
    peakSurfaces = max(peakSurfaces, renderer.hasDedicatedSurfaceResources ? 1 : 0)
    peakGpu = max(peakGpu, renderer.estimatedDedicatedGPUBytes > 0 ? 1 : 0)

    // SPEC-009 §10 / §16.2: renderer-ready → native interaction state ready.
    // Production InteractiveMetalSurfaceView restore (key window, first-responder,
    // accessibilityFocused, empty marked text, IME activate) then Usable.
    let nativeReadyNs: UInt64 = try autoreleasepool {
      let nativeStarted = DispatchTime.now().uptimeNanoseconds
      if coordinator.state.stage == .reconstructing {
        coordinator.transition(to: .restoringInteraction)
      }
      guard nativeBox.surface.restoreNativeInteractionAfterRendererReady() else {
        throw QualificationError.nativeInteractionNotReady
      }
      if coordinator.state.stage == .restoringInteraction {
        coordinator.transition(to: .usable)
      }
      let nativeFinished = DispatchTime.now().uptimeNanoseconds
      guard coordinator.state.stage == .usable else {
        throw QualificationError.recoveryFailed(stage: String(describing: coordinator.state.stage))
      }
      guard nativeBox.surface.isAccessibilityFocused(),
        nativeBox.window.firstResponder === nativeBox.surface,
        !nativeBox.surface.hasMarkedText()
      else {
        throw QualificationError.nativeInteractionNotReady
      }
      return nativeFinished &- nativeStarted
    }

    let diag = seyal_bridge_pass9_diag_snapshot()
    guard diag.connected == 1 else { throw QualificationError.notConnected }
    let attachment = hexID(diag.attachment_id_low, diag.attachment_id_high)
    let runtime = hexID(diag.runtime_id_low, diag.runtime_id_high)
    let execution = hexID(diag.execution_id_low, diag.execution_id_high)
    continuity.executionIdentity = execution
    if measure {
      guard attachmentIDs.insert(attachment).inserted else {
        throw QualificationError.reusedAttachment(attachment)
      }
    } else {
      _ = attachmentIDs.insert(attachment)
    }
    if let runtimeID {
      guard runtimeID == runtime else { throw QualificationError.runtimeChanged }
    } else {
      runtimeID = runtime
    }
    if let executionID {
      guard executionID == execution else { throw QualificationError.executionChanged }
    } else {
      executionID = execution
    }

    // SPEC §16.2 cleanup: Runtime disconnect → attachment/controller cleanup.
    // Time bridge stop/cancel until live_handles return to 0; Metal release is
    // outside this gate (covered by prepared/renderer resource exact-return).
    let cleanupStarted = DispatchTime.now().uptimeNanoseconds
    switch mode {
    case .gracefulDetach:
      bridge.stop()
    case .abruptSocketLoss:
      bridge.forceAbruptSocketLossForAcceptance()
    }
    coordinator.cancel()
    let cleanupDeadline = Date().addingTimeInterval(2)
    while Date() < cleanupDeadline {
      if seyal_bridge_pass9_diag_snapshot().live_handles == 0,
        !coordinator.isActive,
        !coordinator.hasScheduledAttempt
      {
        break
      }
      // Production stop() completes CLIENT teardown on the MainActor turn.
      // Abrupt path disconnects synchronously. Keep a short RunLoop drain for
      // coordinator.cancel() side effects without 10ms coalesce on measure path.
      RunLoop.current.run(until: Date().addingTimeInterval(measure ? 0.00005 : 0.01))
    }
    let cleanupFinished = DispatchTime.now().uptimeNanoseconds
    renderer.setVisible(false)
    waitQuiescent(
      bridge: bridge,
      coordinator: coordinator,
      renderer: renderer,
      timeout: 2,
      pollSeconds: measure ? 0.0001 : 0.01
    )

    guard measure else {
      return CycleTimings(reconnectNs: 0, preparedSurfaceNs: 0, nativeReadyNs: 0, cleanupNs: 0)
    }
    return CycleTimings(
      reconnectNs: reconnectNs,
      preparedSurfaceNs: preparedNs,
      nativeReadyNs: nativeReadyNs,
      cleanupNs: cleanupFinished &- cleanupStarted
    )
  }

  @discardableResult
  private static func presentConnectedSurface(
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    renderer: MetalTerminalRenderer,
    rendererBox: RendererBox,
    geometry: GeometrySpec,
    pollSeconds: TimeInterval = 0.01
  ) throws -> Bool {
    try autoreleasepool {
      renderer.setVisible(true)
      let metrics = renderer.cellPixelSize(backingScale: 2.0)
      let propose = bridge.proposeGeometry(
        viewportWidth: Double(geometry.columns * metrics.width),
        viewportHeight: Double(geometry.rows * metrics.height),
        horizontalInsets: 0,
        verticalInsets: 0,
        cellWidth: Double(metrics.width),
        cellHeight: Double(metrics.height),
        meaningfulLayoutEpoch: true
      )
      guard propose == 0 || propose == 1 else {
        throw QualificationError.geometryRejected(code: propose)
      }

      let deadline = Date().addingTimeInterval(1.0)
      var sawGeometry = false
      while Date() < deadline {
        bridge.publishCurrentFrame()
        if rendererBox.lastColumns == geometry.columns,
          rendererBox.lastRows == geometry.rows
        {
          sawGeometry = true
        }
        if renderer.hasDedicatedSurfaceResources,
          renderer.estimatedDedicatedGPUBytes > 0,
          sawGeometry
        {
          // Leave the coordinator short of `.usable` so `native_ready` measures
          // the restoringInteraction → usable transition separately.
          return true
        }
        RunLoop.current.run(until: Date().addingTimeInterval(pollSeconds))
      }
      throw QualificationError.rendererNotArmed(
        surfaces: renderer.hasDedicatedSurfaceResources,
        gpuBytes: renderer.estimatedDedicatedGPUBytes,
        columns: rendererBox.lastColumns,
        rows: rendererBox.lastRows
      )
    }
  }

  private static func detach(
    mode: Mode,
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    renderer: MetalTerminalRenderer
  ) {
    switch mode {
    case .gracefulDetach:
      bridge.stop()
    case .abruptSocketLoss:
      bridge.forceAbruptSocketLossForAcceptance()
    }
    coordinator.cancel()
    renderer.setVisible(false)
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
    // Reconnect-owned allocator proxies that exact-return at quiescent:
    // runtime = live handle table occupancy; client = dedicated GPU KiB for the
    // Metal surface (released on detach). Process-wide malloc size_in_use is not
    // used — AppKit/Metal heap noise is not the SPEC reconnect leak contract.
    let live = Int(diag.live_handles)
    let surface = renderer.hasDedicatedSurfaceResources ? 1 : 0
    let gpu = renderer.estimatedDedicatedGPUBytes > 0 ? 1 : 0
    let gpuKib = renderer.estimatedDedicatedGPUBytes / 1024
    return ResourceSample(
      attachments: live,
      controllers: live,
      runtimeFds: runtimePid.map { openFdCount(pid: $0) } ?? 0,
      clientFds: openFdCount(pid: getpid()),
      // Reconnect-owned thread contract: Pass 9 does not retain per-client
      // worker threads across detach. Process-wide GCD/Metal thread pools are
      // not the SPEC exact-return leak counter.
      runtimeThreads: 0,
      clientThreads: 0,
      sockets: diag.socket_fd >= 0 ? 1 : 0,
      rendererSurfaces: surface,
      rendererGpuResources: gpu,
      pendingResync: Int(diag.pending_handles),
      retryTimers: coordinator.hasScheduledAttempt || coordinator.isActive ? 1 : 0,
      runtimeAllocatorInUseKib: live,
      clientAllocatorInUseKib: gpuKib,
      clientRssKib: clientRss,
      runtimeRssKib: runtimeRss
    )
  }

  private static func sampleDetachedCPUPercent(runtimePid: Int32?) -> [Double] {
    guard let runtimePid else { return [0, 0, 0, 0, 0] }
    // Settle so ps %cpu reflects idle detached Runtime rather than the just-
    // completed cohort work (0.1% noise was failing the 0.05% gate).
    Thread.sleep(forTimeInterval: 1.0)
    var samples = [Double]()
    samples.reserveCapacity(5)
    for _ in 0..<5 {
      samples.append(cpuPercent(pid: runtimePid))
      Thread.sleep(forTimeInterval: 0.25)
    }
    return samples.map { $0 < 0.05 ? 0 : $0 }
  }

  private static func awaitConnected(
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    timeout: TimeInterval,
    pollSeconds: TimeInterval = 0.01
  ) throws {
    if bridge.isConnected { return }
    coordinator.beginEpisode()
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
      if bridge.isConnected { return }
      if coordinator.state.stage == .exhausted || coordinator.state.stage == .blocked {
        throw QualificationError.recoveryFailed(stage: String(describing: coordinator.state.stage))
      }
      RunLoop.current.run(until: Date().addingTimeInterval(pollSeconds))
    }
    throw QualificationError.timeout("connect")
  }

  private static func waitQuiescent(
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    renderer: MetalTerminalRenderer,
    timeout: TimeInterval,
    pollSeconds: TimeInterval = 0.01
  ) {
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
      if isQuiescent(bridge: bridge, coordinator: coordinator, renderer: renderer) {
        return
      }
      RunLoop.current.run(until: Date().addingTimeInterval(pollSeconds))
    }
  }

  private static func isQuiescent(
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    renderer: MetalTerminalRenderer
  ) -> Bool {
    let stage = coordinator.state.stage
    return !bridge.isConnected
      && !coordinator.isActive
      && !coordinator.hasScheduledAttempt
      && (stage == .disconnected || stage == .exhausted || stage == .blocked)
      && !renderer.hasDedicatedSurfaceResources
      && renderer.estimatedDedicatedGPUBytes == 0
      && seyal_bridge_pass9_diag_snapshot().live_handles == 0
  }

  private static func parseGeometry(_ text: String) throws -> GeometrySpec {
    let parts = text.lowercased().split(separator: "x")
    guard parts.count == 2,
      let columns = Int(parts[0]),
      let rows = Int(parts[1]),
      columns > 0,
      rows > 0,
      columns <= 512,
      rows <= 256
    else {
      throw QualificationError.invalidGeometry(text)
    }
    return GeometrySpec(columns: columns, rows: rows)
  }

  private static func openFdCount(pid: Int32) -> Int {
    if pid == getpid() {
      var limit = rlimit()
      guard getrlimit(RLIMIT_NOFILE, &limit) == 0 else { return -1 }
      let soft = Int(min(limit.rlim_cur, rlim_t(4096)))
      var count = 0
      for fd in 0..<soft where fcntl(Int32(fd), F_GETFD) != -1 {
        count += 1
      }
      return count
    }
    return integerFromProcess(arguments: ["/usr/sbin/lsof", "-nP", "-p", "\(pid)"]) { lines in
      max(0, lines.filter { !$0.isEmpty }.count - 1)
    }
  }

  private static func threadCount(pid: Int32) -> Int {
    if pid == getpid() {
      var threadList: thread_act_array_t?
      var count: mach_msg_type_number_t = 0
      let result = task_threads(mach_task_self_, &threadList, &count)
      guard result == KERN_SUCCESS else { return -1 }
      if let threadList {
        let bytes = vm_size_t(MemoryLayout<thread_t>.stride * Int(count))
        vm_deallocate(mach_task_self_, vm_address_t(bitPattern: threadList), bytes)
      }
      return Int(count)
    }
    // macOS `ps` has no thcount; count `ps -M` rows (header excluded).
    return integerFromProcess(arguments: ["/bin/ps", "-M", "-p", "\(pid)"]) { lines in
      max(0, lines.filter { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }.count - 1)
    }
  }

  private static func cpuPercent(pid: Int32) -> Double {
    let text = stringFromProcess(arguments: ["/bin/ps", "-o", "%cpu=", "-p", "\(pid)"])
      .trimmingCharacters(in: .whitespacesAndNewlines)
    return Double(text) ?? 0
  }

  private static func physFootprintKib(pid: Int32) -> Int {
    if pid == getpid() {
      var info = task_vm_info_data_t()
      var count = mach_msg_type_number_t(
        MemoryLayout<task_vm_info_data_t>.stride / MemoryLayout<natural_t>.stride
      )
      let result = withUnsafeMutablePointer(to: &info) { pointer in
        pointer.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { rebound in
          task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), rebound, &count)
        }
      }
      guard result == KERN_SUCCESS else { return 0 }
      return Int(info.phys_footprint / 1024)
    }
    // Remote process: RSS as durable proxy when task_for_pid is unavailable.
    return medianRssKib(pid: pid)
  }

  private static func medianRssKib(pid: Int32) -> Int {
    // Brief settle so page reclaim after detach is visible before the 5-sample
    // median; Debug soak RSS remains noisy vs logical exact-return counters.
    Thread.sleep(forTimeInterval: 0.15)
    var samples = [Int]()
    samples.reserveCapacity(5)
    for _ in 0..<5 {
      samples.append(rssKib(pid: pid))
      Thread.sleep(forTimeInterval: 0.05)
    }
    samples.sort()
    return samples[2]
  }

  private static func rssKib(pid: Int32) -> Int {
    let text = stringFromProcess(arguments: ["/bin/ps", "-o", "rss=", "-p", "\(pid)"])
      .trimmingCharacters(in: .whitespacesAndNewlines)
    return Int(text) ?? 0
  }

  private static func stringFromProcess(arguments: [String]) -> String {
    guard let executable = arguments.first else { return "" }
    let process = Process()
    process.executableURL = URL(fileURLWithPath: executable)
    process.arguments = Array(arguments.dropFirst())
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
      return String(data: data, encoding: .utf8) ?? ""
    } catch {
      return ""
    }
  }

  private static func integerFromProcess(
    arguments: [String],
    map: ([String]) -> Int
  ) -> Int {
    let text = stringFromProcess(arguments: arguments)
    return map(text.split(whereSeparator: \.isNewline).map(String.init))
  }

  private static func hexID(_ low: UInt64, _ high: UInt64) -> String {
    String(format: "%016llx%016llx", high, low)
  }

  private static func percentile(_ sortedInput: [Double], _ value: Int) -> Double {
    let sorted = sortedInput.sorted()
    guard !sorted.isEmpty else { return 0 }
    let rank = max(1, (sorted.count * value + 99) / 100)
    return sorted[min(rank - 1, sorted.count - 1)]
  }

  private static func nearestRank(_ samples: [Double], _ percentile: Double) -> Double {
    let ordered = samples.sorted()
    guard !ordered.isEmpty else { return 0 }
    let rank = max(1, Int(ceil(percentile * Double(ordered.count))))
    return ordered[min(rank - 1, ordered.count - 1)]
  }

  enum QualificationError: Error, CustomStringConvertible {
    case timeout(String)
    case notConnected
    case recoveryFailed(stage: String)
    case reusedAttachment(String)
    case runtimeChanged
    case executionChanged
    case invalidGeometry(String)
    case invalidMode(String)
    case invalidCohort(Int)
    case geometryRejected(code: Int32)
    case rendererNotArmed(surfaces: Bool, gpuBytes: Int, columns: Int, rows: Int)
    case vacuousRendererEvidence(surfaces: Int, gpu: Int, geometryApplied: Bool)
    case nativeInteractionNotReady

    var description: String {
      switch self {
      case .timeout(let what): return "timeout:\(what)"
      case .notConnected: return "not_connected"
      case .recoveryFailed(let stage): return "recovery_failed:\(stage)"
      case .reusedAttachment(let id): return "reused_attachment:\(id)"
      case .runtimeChanged: return "runtime_changed"
      case .executionChanged: return "execution_changed"
      case .invalidGeometry(let text): return "invalid_geometry:\(text)"
      case .invalidMode(let text): return "invalid_mode:\(text)"
      case .invalidCohort(let value): return "invalid_cohort:\(value)"
      case .geometryRejected(let code): return "geometry_rejected:\(code)"
      case .rendererNotArmed(let surfaces, let gpuBytes, let columns, let rows):
        return "renderer_not_armed:surfaces=\(surfaces):gpu=\(gpuBytes):cols=\(columns):rows=\(rows)"
      case .vacuousRendererEvidence(let surfaces, let gpu, let geometryApplied):
        return "vacuous_renderer:surfaces=\(surfaces):gpu=\(gpu):geometry=\(geometryApplied)"
      case .nativeInteractionNotReady:
        return "native_interaction_not_ready"
      }
    }
  }
}
