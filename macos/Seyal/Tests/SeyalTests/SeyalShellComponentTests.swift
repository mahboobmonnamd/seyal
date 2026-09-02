import AppKit
import Darwin
import XCTest

@testable import Seyal

final class SeyalShellComponentTests: XCTestCase {

  func testBundledRuntimeEnvironmentIsExactAllowlistAndRejectsPoisonedValues() throws {
    let environment = try BundledRuntimeLauncher.launchEnvironment(inherited: [
      "LANG": "en_US.UTF-8",
      "LC_CTYPE": "bad\nvalue",
      "DYLD_INSERT_LIBRARIES": "/tmp/injected.dylib",
      "SSH_AUTH_SOCK": "/tmp/agent.sock",
      "SEYAL_SECRET": "secret",
    ])

    XCTAssertEqual(
      Set(environment.keys),
      Set(["HOME", "USER", "LOGNAME", "SHELL", "TMPDIR", "PATH", "LANG"])
    )
    XCTAssertEqual(environment["PATH"], "/usr/bin:/bin:/usr/sbin:/sbin")
    XCTAssertEqual(environment["USER"], environment["LOGNAME"])
    XCTAssertEqual(environment["LANG"], "en_US.UTF-8")
    XCTAssertNil(environment["LC_CTYPE"])
    XCTAssertNil(environment["DYLD_INSERT_LIBRARIES"])
    XCTAssertNil(environment["SSH_AUTH_SOCK"])
    XCTAssertNil(environment["SEYAL_SECRET"])
  }

  func testBundledRuntimeLocaleValidationIsBoundedAndControlFree() {
    XCTAssertTrue(BundledRuntimeLauncher.isValidLocale("en_US.UTF-8"))
    XCTAssertFalse(BundledRuntimeLauncher.isValidLocale(""))
    XCTAssertFalse(BundledRuntimeLauncher.isValidLocale("en_US\nUTF-8"))
    XCTAssertFalse(BundledRuntimeLauncher.isValidLocale(String(repeating: "x", count: 129)))
  }

  func testBundledRuntimePathAcceptsOnlyExactRegularExecutableHelper() throws {
    let root = FileManager.default.temporaryDirectory
      .appendingPathComponent(UUID().uuidString, isDirectory: true)
    let bundle = root.appendingPathComponent("Seyal.app", isDirectory: true)
    let helpers = bundle.appendingPathComponent("Contents/Helpers", isDirectory: true)
    let helper = helpers.appendingPathComponent("seyal-runtime")
    try FileManager.default.createDirectory(at: helpers, withIntermediateDirectories: true)
    XCTAssertThrowsError(try BundledRuntimeLauncher.validateHelperPath(bundleURL: bundle)) {
      XCTAssertEqual($0 as? BundledRuntimeLaunchError, .helperMissing)
    }

    XCTAssertTrue(FileManager.default.createFile(atPath: helper.path, contents: Data("runtime".utf8)))
    try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: helper.path)
    XCTAssertEqual(
      try BundledRuntimeLauncher.validateHelperPath(bundleURL: bundle),
      helper.standardizedFileURL
    )

    try FileManager.default.removeItem(at: helper)
    try FileManager.default.createSymbolicLink(at: helper, withDestinationURL: URL(fileURLWithPath: "/bin/true"))
    XCTAssertThrowsError(try BundledRuntimeLauncher.validateHelperPath(bundleURL: bundle)) {
      XCTAssertEqual($0 as? BundledRuntimeLaunchError, .helperPathInvalid)
    }
    try? FileManager.default.removeItem(at: root)
  }

  /// SPEC-009 8.1.1 requires canary evidence that `spawn()` genuinely closes
  /// every descriptor >= 3 and runs the helper in its own process group, and
  /// requires `validateCodeSignature()` to accept a correctly ad-hoc-signed
  /// helper/app pair (the Debug-only trust path this host can produce without
  /// a paid Apple Developer signing identity). No prior test in this diff
  /// exercised `validateCodeSignature()` or `spawn()` at all; both were only
  /// unit-tested for `validateHelperPath`/`launchEnvironment` in isolation.
  func testBundledRuntimeSpawnClosesInheritedDescriptorsAndUsesOwnProcessGroup() throws {
    let root = FileManager.default.temporaryDirectory
      .appendingPathComponent(UUID().uuidString, isDirectory: true)
    let bundle = root.appendingPathComponent("Seyal.app", isDirectory: true)
    let macOS = bundle.appendingPathComponent("Contents/MacOS", isDirectory: true)
    let helpers = bundle.appendingPathComponent("Contents/Helpers", isDirectory: true)
    let helper = helpers.appendingPathComponent("seyal-runtime")
    try FileManager.default.createDirectory(at: helpers, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: macOS, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }

    // codesign refuses to treat a directory as a bundle unless it looks like
    // one: a recognizable Info.plist and a main executable under
    // Contents/MacOS. Neither needs real content for an ad-hoc signature.
    let infoPlist: [String: Any] = [
      "CFBundleIdentifier": "dev.seyal.Seyal.test-fixture-app",
      "CFBundleExecutable": "Seyal",
      "CFBundlePackageType": "APPL",
    ]
    let plistData = try PropertyListSerialization.data(
      fromPropertyList: infoPlist, format: .xml, options: 0)
    try plistData.write(to: bundle.appendingPathComponent("Contents/Info.plist"))
    try FileManager.default.copyItem(
      at: URL(fileURLWithPath: "/bin/echo"),
      to: macOS.appendingPathComponent("Seyal"))

    let marker = FileManager.default.temporaryDirectory
      .appendingPathComponent("seyal-canary-\(UUID().uuidString).txt")
    defer { try? FileManager.default.removeItem(at: marker) }

    // The helper never receives arguments (spawn() uses an empty argv), so
    // the probe logic must live in the executed file itself. /dev/fd lets a
    // plain shell script enumerate its own open descriptors without needing
    // a second interpreter or a channel outside the closed environment
    // allowlist: the marker path is baked into the script text, not passed
    // through the environment.
    let script = """
      #!/bin/sh
      OUT="\(marker.path)"
      : > "$OUT"
      echo "pid=$$" >> "$OUT"
      echo "pgid=$(ps -o pgid= -p $$ | tr -d ' ')" >> "$OUT"
      for fd in $(seq 3 768); do
        if [ -e "/dev/fd/$fd" ]; then
          echo "open_fd=$fd" >> "$OUT"
        fi
      done
      exit 0
      """
    try script.write(to: helper, atomically: true, encoding: .utf8)
    try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: helper.path)

    // Sign the helper with its own identity before sealing the app (non-deep,
    // so the app's own ad-hoc signature does not overwrite the helper's
    // distinct dev.seyal.Seyal.runtime identifier).
    try adHocSign(helper, identifier: BundledRuntimeLauncher.helperIdentifier)
    try adHocSign(bundle, identifier: "dev.seyal.Seyal.test-fixture-app")

    // Hold open several descriptors at fd numbers a live GUI process would
    // plausibly have in real use (sockets/log files stand-in). posix_spawn's
    // close-all default must exclude every one of these from the child
    // regardless of which numbers the kernel happened to assign them.
    let canaries = (0..<6).map { _ in Pipe() }
    let canaryDescriptors = Set(
      canaries.flatMap {
        [$0.fileHandleForReading.fileDescriptor, $0.fileHandleForWriting.fileDescriptor]
      }
    )
    defer {
      canaries.forEach {
        $0.fileHandleForReading.closeFile()
        $0.fileHandleForWriting.closeFile()
      }
    }

    let result = BundledRuntimeLauncher().launch(bundleURL: bundle)
    guard case .success(let pid) = result else {
      XCTFail("expected an ad-hoc-signed Debug helper/app pair to launch, got \(result)")
      return
    }

    let deadline = Date().addingTimeInterval(5)
    while !FileManager.default.fileExists(atPath: marker.path), Date() < deadline {
      RunLoop.current.run(until: Date().addingTimeInterval(0.05))
    }
    // BundledRuntimeLauncher's own reaper runs asynchronously on a background
    // queue; this test reaps independently so it does not depend on that
    // timing, matching how any other same-UID waiter could observe the PID.
    var status: Int32 = 0
    while waitpid(pid, &status, 0) == -1 && errno == EINTR {}

    let contents = try String(contentsOf: marker, encoding: .utf8)
    let lines = contents.split(separator: "\n").map(String.init)

    guard let pidLine = lines.first(where: { $0.hasPrefix("pid=") }),
      let reportedPID = pid_t(pidLine.dropFirst("pid=".count))
    else {
      XCTFail("canary helper did not report its own pid; raw output: \(contents)")
      return
    }
    XCTAssertEqual(reportedPID, pid)

    guard let pgidLine = lines.first(where: { $0.hasPrefix("pgid=") }),
      let reportedPGID = pid_t(pgidLine.dropFirst("pgid=".count))
    else {
      XCTFail("canary helper did not report its process group; raw output: \(contents)")
      return
    }
    XCTAssertEqual(
      reportedPGID, pid,
      "helper must run in its own process group, never the GUI's"
    )

    let openInChild = Set(
      lines.filter { $0.hasPrefix("open_fd=") }
        .compactMap { Int($0.dropFirst("open_fd=".count)) }
    )
    let leaked = openInChild.intersection(canaryDescriptors.map(Int.init))
    XCTAssertTrue(
      leaked.isEmpty,
      "posix_spawn must close every inherited descriptor >= 3; leaked \(leaked) into the helper"
    )
  }

  private func adHocSign(_ url: URL, identifier: String) throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/codesign")
    process.arguments = ["--force", "--sign", "-", "--identifier", identifier, url.path]
    let stderrPipe = Pipe()
    process.standardError = stderrPipe
    try process.run()
    process.waitUntilExit()
    if process.terminationStatus != 0 {
      let output = String(
        data: stderrPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
      XCTFail("ad-hoc codesign of \(url.path) failed: \(output)")
    }
  }

  @MainActor
  private final class RecoveryScheduler {
    final class Job: @unchecked Sendable {
      let delay: TimeInterval
      let operation: RuntimeLifecycleRecoveryCoordinator.ScheduledOperation
      var cancelled = false

      init(
        delay: TimeInterval,
        operation: @escaping RuntimeLifecycleRecoveryCoordinator.ScheduledOperation
      ) {
        self.delay = delay
        self.operation = operation
      }
    }

    var now: TimeInterval = 0
    private(set) var jobs: [Job] = []

    func schedule(
      delay: TimeInterval,
      operation: @escaping RuntimeLifecycleRecoveryCoordinator.ScheduledOperation
    ) -> @Sendable () -> Void {
      let job = Job(delay: delay, operation: operation)
      jobs.append(job)
      return { job.cancelled = true }
    }

    func fire(_ index: Int, includingCancelled: Bool = false) {
      let job = jobs[index]
      now += job.delay
      if includingCancelled || !job.cancelled {
        job.operation()
      }
    }
  }

  @MainActor
  func testLifecycleCoordinatorUsesExactSevenAttemptScheduleAndExhausts() {
    let scheduler = RecoveryScheduler()
    var attempts = 0
    let coordinator = RuntimeLifecycleRecoveryCoordinator(
      clock: { scheduler.now },
      scheduler: { delay, operation in
        MainActor.assumeIsolated {
          scheduler.schedule(delay: delay, operation: operation)
        }
      },
      launcher: {},
      attempt: {
        attempts += 1
        return .retryable
      },
      attemptExecution: .inline
    )

    coordinator.beginEpisode()
    for index in RuntimeLifecycleRecoveryCoordinator.retryDelays.indices {
      scheduler.fire(index)
    }

    XCTAssertEqual(attempts, 7)
    XCTAssertEqual(coordinator.attemptCount, 7)
    XCTAssertEqual(scheduler.jobs.map(\.delay), [0.010, 0.020, 0.040, 0.080, 0.160, 0.250])
    XCTAssertEqual(coordinator.state.stage, .exhausted)
    XCTAssertFalse(coordinator.hasScheduledAttempt)
  }

  @MainActor
  func testLifecycleCoordinatorLaunchesOnceAndControllerBusyNeverLaunches() {
    let missingScheduler = RecoveryScheduler()
    var launches = 0
    let missing = RuntimeLifecycleRecoveryCoordinator(
      clock: { missingScheduler.now },
      scheduler: { delay, operation in
        MainActor.assumeIsolated {
          missingScheduler.schedule(delay: delay, operation: operation)
        }
      },
      launcher: { launches += 1 },
      attempt: { .endpointMissing },
      attemptExecution: .inline
    )
    missing.beginEpisode()
    for index in RuntimeLifecycleRecoveryCoordinator.retryDelays.indices {
      missingScheduler.fire(index)
    }
    XCTAssertEqual(launches, 1)
    XCTAssertEqual(missing.state.stage, .exhausted)

    let busyScheduler = RecoveryScheduler()
    let busy = RuntimeLifecycleRecoveryCoordinator(
      clock: { busyScheduler.now },
      scheduler: { delay, operation in
        MainActor.assumeIsolated {
          busyScheduler.schedule(delay: delay, operation: operation)
        }
      },
      launcher: { launches += 1 },
      attempt: { .controllerBusy },
      attemptExecution: .inline
    )
    busy.beginEpisode()
    XCTAssertEqual(busy.state.stage, .waitingForController)
    for index in RuntimeLifecycleRecoveryCoordinator.retryDelays.indices {
      busyScheduler.fire(index)
    }
    XCTAssertEqual(launches, 1)
    XCTAssertEqual(busy.attemptCount, 7)
    XCTAssertEqual(busy.state.stage, .exhausted)
  }

  @MainActor
  func testLifecycleCoordinatorDeadlineCancellationAndGenerationReplacement() {
    let scheduler = RecoveryScheduler()
    var attempts = 0
    let coordinator = RuntimeLifecycleRecoveryCoordinator(
      clock: { scheduler.now },
      scheduler: { delay, operation in
        MainActor.assumeIsolated {
          scheduler.schedule(delay: delay, operation: operation)
        }
      },
      launcher: {},
      attempt: {
        attempts += 1
        return .retryable
      },
      attemptExecution: .inline
    )

    coordinator.beginEpisode()
    let firstGeneration = coordinator.state.generation
    coordinator.beginEpisode()
    XCTAssertGreaterThan(coordinator.state.generation, firstGeneration)
    XCTAssertEqual(attempts, 2)
    scheduler.fire(0, includingCancelled: true)
    XCTAssertEqual(attempts, 2, "stale generation callback must not attempt")

    scheduler.now = 1.0
    scheduler.fire(1)
    XCTAssertEqual(attempts, 2, "deadline must stop before another attempt")
    XCTAssertEqual(coordinator.state.stage, .exhausted)
    XCTAssertFalse(coordinator.hasScheduledAttempt)

    let exhaustedGeneration = coordinator.state.generation
    coordinator.retry()
    XCTAssertGreaterThan(coordinator.state.generation, exhaustedGeneration)
    XCTAssertEqual(attempts, 3)
    coordinator.cancel()
    XCTAssertEqual(coordinator.state.stage, .disconnected)
    XCTAssertFalse(coordinator.hasScheduledAttempt)
  }

  @MainActor
  func testLifecycleCoordinatorSuccessCancelsOutstandingRecovery() {
    let scheduler = RecoveryScheduler()
    var outcomes: [RuntimeRecoveryAttemptOutcome] = [.retryable, .connected]
    let coordinator = RuntimeLifecycleRecoveryCoordinator(
      clock: { scheduler.now },
      scheduler: { delay, operation in
        MainActor.assumeIsolated {
          scheduler.schedule(delay: delay, operation: operation)
        }
      },
      launcher: {},
      attempt: { outcomes.removeFirst() },
      attemptExecution: .inline
    )

    coordinator.beginEpisode()
    scheduler.fire(0)
    XCTAssertEqual(coordinator.state.stage, .reconstructing)
    XCTAssertFalse(coordinator.hasScheduledAttempt)
    XCTAssertFalse(coordinator.isActive)
  }

  func testReconnectReconstructionPinsRuntimeExecutionAndRequiresFreshAttachment() {
    let runtime = RuntimeContinuityIdentity(low: 1, high: 2)
    let execution = RuntimeContinuityIdentity(low: 3, high: 4)
    let firstAttachment = RuntimeContinuityIdentity(low: 5, high: 6)
    let secondAttachment = RuntimeContinuityIdentity(low: 7, high: 8)
    var state = ReconnectReconstructionState()

    state.beginAttempt()
    XCTAssertFalse(state.canMutate)
    XCTAssertTrue(state.commit(
      runtime: runtime,
      execution: execution,
      attachment: firstAttachment,
      controllerAuthorityCommitted: true,
      authoritativeSnapshotCommitted: true
    ))
    XCTAssertTrue(state.canMutate)
    state.disconnect()
    state.beginAttempt()
    XCTAssertFalse(state.canMutate)
    XCTAssertTrue(state.commit(
      runtime: runtime,
      execution: execution,
      attachment: secondAttachment,
      controllerAuthorityCommitted: true,
      authoritativeSnapshotCommitted: true
    ))

    state.disconnect()
    state.beginAttempt()
    XCTAssertFalse(state.commit(
      runtime: RuntimeContinuityIdentity(low: 99, high: 2),
      execution: execution,
      attachment: RuntimeContinuityIdentity(low: 9, high: 10),
      controllerAuthorityCommitted: true,
      authoritativeSnapshotCommitted: true
    ))
    XCTAssertEqual(state.stage, .blockedIdentityMismatch)
    XCTAssertFalse(state.canMutate)
  }

  func testReconnectReconstructionRejectsInterruptedSnapshotAndOldAttachment() {
    let runtime = RuntimeContinuityIdentity(low: 1, high: 2)
    let execution = RuntimeContinuityIdentity(low: 3, high: 4)
    let attachment = RuntimeContinuityIdentity(low: 5, high: 6)
    var state = ReconnectReconstructionState()

    state.beginAttempt()
    XCTAssertFalse(state.commit(
      runtime: runtime,
      execution: execution,
      attachment: attachment,
      controllerAuthorityCommitted: true,
      authoritativeSnapshotCommitted: false
    ))
    XCTAssertEqual(state.stage, .awaitingAuthoritativeSnapshot)
    XCTAssertFalse(state.canMutate)

    XCTAssertTrue(state.commit(
      runtime: runtime,
      execution: execution,
      attachment: attachment,
      controllerAuthorityCommitted: true,
      authoritativeSnapshotCommitted: true
    ))
    state.disconnect()
    state.beginAttempt()
    XCTAssertFalse(state.commit(
      runtime: runtime,
      execution: execution,
      attachment: attachment,
      controllerAuthorityCommitted: true,
      authoritativeSnapshotCommitted: true
    ))
    XCTAssertEqual(state.stage, .blockedIdentityMismatch)
    XCTAssertFalse(state.canMutate)
  }

  func testRuntimeBlockMetadataKeepsOpaqueExecutionIdentityAnchorAndState() {
    let current = RuntimeBlockMetadata(
      blockIDLow: 0x0123,
      blockIDHigh: 0x4567,
      revision: 1,
      startLineID: 99,
      state: .current
    )
    let completed = RuntimeBlockMetadata(
      blockIDLow: current.blockIDLow,
      blockIDHigh: current.blockIDHigh,
      revision: 2,
      startLineID: current.startLineID,
      state: .completed
    )

    XCTAssertEqual(current.blockIDLow, completed.blockIDLow)
    XCTAssertEqual(current.blockIDHigh, completed.blockIDHigh)
    XCTAssertEqual(current.startLineID, completed.startLineID)
    XCTAssertEqual(current.revision, 1)
    XCTAssertEqual(completed.revision, 2)
    XCTAssertEqual(current.state, .current)
    XCTAssertEqual(completed.state, .completed)
    XCTAssertNotEqual(current, completed)
  }
  func testExecutionBlockMetadataCABIIsStable() {
    XCTAssertEqual(MemoryLayout<SeyalExecutionBlockMetadata>.size, 40)
    XCTAssertEqual(MemoryLayout<SeyalExecutionBlockMetadata>.stride, 40)
    XCTAssertEqual(MemoryLayout<SeyalExecutionBlockMetadata>.alignment, 8)
  }

  func testPaneQualifiedIdentitiesDoNotCollideAcrossPanes() {
    let firstBlock = PaneBlockKey(paneID: "pane-left", blockID: 7)
    let secondBlock = PaneBlockKey(paneID: "pane-right", blockID: 7)
    let firstRequest = PaneHistoryRequestKey(paneID: "pane-left", requestID: 19)
    let secondRequest = PaneHistoryRequestKey(paneID: "pane-right", requestID: 19)

    XCTAssertNotEqual(firstBlock, secondBlock)
    XCTAssertNotEqual(firstRequest, secondRequest)
    XCTAssertEqual(Set([firstBlock, secondBlock]).count, 2)
    XCTAssertEqual(Set([firstRequest, secondRequest]).count, 2)
    XCTAssertNotEqual(firstBlock.accessibilityIdentifier, secondBlock.accessibilityIdentifier)
  }

  @MainActor
  func testExplicitExecutionIdentityKeepsPaneHandlesIndependent() {
    let left = RustDisplayBridge.executionWords(from: "00112233445566778899aabbccddeeff")
    let right = RustDisplayBridge.executionWords(from: "ffeeddccbbaa99880099aabbccddeeff")

    XCTAssertEqual(left?.0, 0x8899_aabb_ccdd_eeff)
    XCTAssertEqual(left?.1, 0x0011_2233_4455_6677)
    XCTAssertEqual(right?.0, 0x0099_aabb_ccdd_eeff)
    XCTAssertEqual(right?.1, 0xffee_ddcc_bbaa_9988)
    XCTAssertNotEqual(left?.0, right?.0)
    XCTAssertNotEqual(left?.1, right?.1)
    XCTAssertNil(RustDisplayBridge.executionWords(from: "not-an-execution"))
  }

  @MainActor
  func testProductionPanesRetainExplicitExecutionIdentityAndDoNotBootstrapImplicitly() {
    let left = SeyalShellState.Pane(
      id: "pane-left",
      title: "Left",
      executionIdentity: "00112233445566778899aabbccddeeff",
      allowsImplicitExecutionBootstrap: false
    )
    let right = SeyalShellState.Pane(
      id: "pane-right",
      title: "Right",
      executionIdentity: "ffeeddccbbaa99887766554433221100",
      allowsImplicitExecutionBootstrap: false
    )

    XCTAssertEqual(left.executionIdentity, "00112233445566778899aabbccddeeff")
    XCTAssertEqual(right.executionIdentity, "ffeeddccbbaa99887766554433221100")
    XCTAssertFalse(left.allowsImplicitExecutionBootstrap)
    XCTAssertFalse(right.allowsImplicitExecutionBootstrap)
    XCTAssertNotEqual(left.executionIdentity, right.executionIdentity)

    let leftTranscript = PaneTranscriptView(
      paneID: left.id,
      installSurface: false,
      executionIdentity: left.executionIdentity,
      allowsImplicitExecutionBootstrap: left.allowsImplicitExecutionBootstrap
    )
    let rightTranscript = PaneTranscriptView(
      paneID: right.id,
      installSurface: false,
      executionIdentity: right.executionIdentity,
      allowsImplicitExecutionBootstrap: right.allowsImplicitExecutionBootstrap
    )
    XCTAssertEqual(leftTranscript.terminalSurface.requestedExecutionIdentity, left.executionIdentity)
    XCTAssertEqual(rightTranscript.terminalSurface.requestedExecutionIdentity, right.executionIdentity)
    XCTAssertFalse(leftTranscript.terminalSurface.allowsImplicitExecutionBootstrap)
    XCTAssertFalse(rightTranscript.terminalSurface.allowsImplicitExecutionBootstrap)
    XCTAssertNotEqual(leftTranscript.terminalSurface, rightTranscript.terminalSurface)
  }

  @MainActor
  func testInterleavedPaneHistoryRetentionDoesNotEvictForeignRequests() {
    let leftBlock = PaneBlockKey(paneID: "pane-left", blockID: 7)
    let rightBlock = PaneBlockKey(paneID: "pane-right", blockID: 7)
    let rightOtherBlock = PaneBlockKey(paneID: "pane-right", blockID: 8)
    let retained = SeyalShellView.retainedHistoryKeys(
      existing: [leftBlock, rightBlock, rightOtherBlock],
      paneID: "pane-left",
      retainedBlockIDs: [7]
    )

    XCTAssertEqual(retained, [leftBlock, rightBlock, rightOtherBlock])

    let afterLeftEviction = SeyalShellView.retainedHistoryKeys(
      existing: retained,
      paneID: "pane-left",
      retainedBlockIDs: []
    )
    XCTAssertEqual(afterLeftEviction, [rightBlock, rightOtherBlock])
  }

  @MainActor
  func testPaneQualifiedConstraintOwnershipKeepsIdenticalBlockIDsIndependent() {
    let host = NSView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
    let left = NSView()
    let right = NSView()
    host.addSubview(left)
    host.addSubview(right)
    let leftConstraint = left.widthAnchor.constraint(equalToConstant: 40)
    let rightConstraint = right.widthAnchor.constraint(equalToConstant: 60)
    let ownership = KeyedConstraintOwnership()
    let leftKey = PaneBlockKey(paneID: "pane-left", blockID: 7)
    let rightKey = PaneBlockKey(paneID: "pane-right", blockID: 7)

    ownership.install([leftConstraint], for: leftKey)
    ownership.install([rightConstraint], for: rightKey)
    XCTAssertEqual(ownership.count, 2)

    ownership.remove(leftKey)

    XCTAssertFalse(leftConstraint.isActive)
    XCTAssertTrue(rightConstraint.isActive)
    XCTAssertTrue(ownership.contains(rightKey))
  }

  @MainActor
  func testPaneTranscriptCachesKeepIdenticalBlockAndAnchorRangesIndependent() {
    let left = PaneTranscriptView(paneID: "pane-left", installSurface: false)
    let right = PaneTranscriptView(paneID: "pane-right", installSurface: false)
    let leftBody = CommandBlockBodyView()
    let rightBody = CommandBlockBodyView()
    left.registerBlockBody(leftBody, blockID: 7)
    right.registerBlockBody(rightBody, blockID: 7)

    let leftRange = NativeHistoryRange(
      startLine: 11, endLine: 12, blockID: 7, requestID: 19,
      revision: 1,
      rows: [[NativeHistoryRange.Cell(scalar: 76, foreground: 0, background: 0, flags: 0)]]
    )
    let rightRange = NativeHistoryRange(
      startLine: 11, endLine: 12, blockID: 7, requestID: 19,
      revision: 1,
      rows: [[NativeHistoryRange.Cell(scalar: 82, foreground: 0, background: 0, flags: 0)]]
    )
    leftBody.setHistoryRange(leftRange)
    rightBody.setHistoryRange(rightRange)

    XCTAssertEqual(leftBody.historyRange, leftRange)
    XCTAssertEqual(rightBody.historyRange, rightRange)
    XCTAssertNotEqual(left.terminalSurface, right.terminalSurface)
  }

  @MainActor
  func testBlockOwnsNoNestedScrollView() {
    let body = NSView()
    body.translatesAutoresizingMaskIntoConstraints = false
    body.heightAnchor.constraint(equalToConstant: 120).isActive = true

    let presentation = BlockPresentation(
      id: "component-test",
      command: "make test",
      state: BlockPresentationState.completed,
      elapsed: "12 ms",
      timestamp: "09:00",
      isSelected: true,
      actions: ["Copy", "Pin"]
    )
    let block = BlockView(presentation: presentation, bodyView: body)

    XCTAssertTrue(descendants(of: NSScrollView.self, in: block).isEmpty)
    XCTAssertTrue(block.subviewsRecursively.contains { $0 === body })
  }

  @MainActor
  func testCommandBlockBodyNeverCopiesTerminalCellsIntoTextView() {
    let body = CommandBlockBodyView()

    XCTAssertTrue(
      descendants(of: NSTextView.self, in: body).isEmpty,
      "normal Flow Blocks must render canonical terminal projection, not a copied NSTextView transcript"
    )
  }

  @MainActor
  func testCommandBlockBodyHasIntrinsicHeightBeforeAndAfterCanonicalRows() {
    let body = CommandBlockBodyView()
    XCTAssertGreaterThan(body.intrinsicContentSize.height, 0)

    let range = NativeHistoryRange(
      startLine: 11,
      endLine: 13,
      blockID: 7,
      requestID: 19,
      revision: 2,
      rows: [
        [NativeHistoryRange.Cell(scalar: 65, foreground: 0, background: 0, flags: 0)],
        [NativeHistoryRange.Cell(scalar: 66, foreground: 0, background: 0, flags: 0)],
        [NativeHistoryRange.Cell(scalar: 67, foreground: 0, background: 0, flags: 0)],
      ]
    )
    body.setHistoryRange(range)

    XCTAssertEqual(body.historyRange, range)
    XCTAssertGreaterThanOrEqual(body.intrinsicContentSize.height, 3 * body.rowHeight)
  }

  @MainActor
  func testHistoryRangeGrowthNotifiesPaneForCompleteOrderedFrame() {
    let transcript = PaneTranscriptView(installSurface: false)
    let body = CommandBlockBodyView()
    var refreshes = 0
    transcript.onBlockBodySizeChanged = { refreshes += 1 }
    transcript.registerBlockBody(body, blockID: 7)

    body.setHistoryRange(
      NativeHistoryRange(
        startLine: 11,
        endLine: 11,
        blockID: 7,
        requestID: 19,
        revision: 1,
        rows: [[NativeHistoryRange.Cell(scalar: 65, foreground: 0, background: 0, flags: 0)]]
      ))
    body.setHistoryRange(
      NativeHistoryRange(
        startLine: 11,
        endLine: 13,
        blockID: 7,
        requestID: 19,
        revision: 2,
        rows: [
          [NativeHistoryRange.Cell(scalar: 65, foreground: 0, background: 0, flags: 0)],
          [NativeHistoryRange.Cell(scalar: 66, foreground: 0, background: 0, flags: 0)],
          [NativeHistoryRange.Cell(scalar: 67, foreground: 0, background: 0, flags: 0)],
        ]
      ))

    XCTAssertEqual(refreshes, 2)
    XCTAssertEqual(transcript.transcriptFrame().regionIDs, [7])
  }

  @MainActor
  func testBlockAppliesAuthoritativeLifecycleWithoutReplacingView() {
    let body = NSView()
    body.translatesAutoresizingMaskIntoConstraints = false
    body.heightAnchor.constraint(equalToConstant: 24).isActive = true
    let block = BlockView(
      presentation: BlockPresentation(
        id: "stable", command: "echo old", state: .running, elapsed: "Live",
        timestamp: nil, isSelected: true, actions: []
      ),
      bodyView: body
    )

    let originalBody = block.subviewsRecursively.first { $0 === body }
    block.apply(
      presentation: BlockPresentation(
        id: "stable", command: "echo old", state: .completed, elapsed: "Done",
        timestamp: nil, isSelected: false, actions: []
      ))

    XCTAssertTrue(block.subviewsRecursively.contains { $0 === body })
    XCTAssertTrue(block.subviewsRecursively.contains { $0 === originalBody })
    XCTAssertEqual(block.presentationState, .completed)
  }

  @MainActor
  func testPaneTranscriptOwnsOneDocumentAndOneSurface() {
    let transcript = PaneTranscriptView()
    transcript.layoutSubtreeIfNeeded()

    XCTAssertNotNil(transcript.documentView)
    XCTAssertEqual(descendants(of: NSScrollView.self, in: transcript).count, 0)
    XCTAssertEqual(descendants(of: InteractiveMetalSurfaceView.self, in: transcript).count, 1)
  }

  @MainActor
  func testPaneTranscriptRegistersAllBlockRegionsOnOneSurfaceInLifecycleOrder() {
    let transcript = PaneTranscriptView(installSurface: false)
    let first = CommandBlockBodyView()
    let second = CommandBlockBodyView()
    transcript.registerBlockBody(first, blockID: 11)
    transcript.registerBlockBody(second, blockID: 12)

    let frame = transcript.transcriptFrame()

    XCTAssertEqual(frame.regionIDs, [11, 12])
    XCTAssertEqual(frame.surfaceIdentity, ObjectIdentifier(transcript.terminalSurface))
    XCTAssertTrue(frame.regions.allSatisfy { $0.clip.width >= 0 && $0.clip.height >= 0 })
  }

  @MainActor
  func testNativeTranscriptFrameRejectsDuplicateIDsAndKeepsAtomicRevision() {
    let valid = NativeTranscriptFrame(
      revision: 9,
      regions: [
        NativeTranscriptRegion(
          id: 21, origin: .zero, clip: NSRect(x: 0, y: 0, width: 10, height: 10)),
        NativeTranscriptRegion(
          id: 22, origin: NSPoint(x: 0, y: 10), clip: NSRect(x: 0, y: 10, width: 10, height: 10)),
      ]
    )
    XCTAssertTrue(valid.isValid)
    XCTAssertFalse(valid.applyingDuplicateID(22).isValid)
    XCTAssertEqual(valid.revision, 9)
  }

  @MainActor
  func testKeyedBlockConstraintsDeactivateOnlyWhenTheirBlockIsEvicted() {
    let host = NSView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
    let first = NSView()
    let second = NSView()
    host.addSubview(first)
    host.addSubview(second)
    let firstConstraint = first.widthAnchor.constraint(equalToConstant: 40)
    let secondConstraint = second.widthAnchor.constraint(equalToConstant: 60)
    let ownership = KeyedConstraintOwnership()

    let firstKey = PaneBlockKey(paneID: "pane-test", blockID: 1)
    let secondKey = PaneBlockKey(paneID: "pane-test", blockID: 2)
    ownership.install([firstConstraint], for: firstKey)
    ownership.install([secondConstraint], for: secondKey)
    XCTAssertEqual(ownership.count, 2)
    XCTAssertTrue(firstConstraint.isActive)
    XCTAssertTrue(secondConstraint.isActive)

    ownership.remove(firstKey)

    XCTAssertEqual(ownership.count, 1)
    XCTAssertFalse(firstConstraint.isActive)
    XCTAssertTrue(secondConstraint.isActive)
    ownership.remove(PaneBlockKey(paneID: "pane-test", blockID: 99))
    XCTAssertEqual(ownership.count, 1)
  }

  @MainActor
  func testComposerResultCorrelationOnlyAcceptsMatchingRequest() {
    var correlation = ComposerRequestCorrelation()
    let request = correlation.begin(command: "printf one")
    XCTAssertFalse(correlation.accepts(requestID: request + 1))
    XCTAssertFalse(correlation.isSettled)
    XCTAssertTrue(correlation.accepts(requestID: request))
    XCTAssertTrue(correlation.isSettled)
  }

  @MainActor
  func testComposerCorrelationSettlesIdenticalCommandsByRequestIDOnly() {
    var correlation = ComposerRequestCorrelation()
    let first = correlation.begin(command: "printf same")
    XCTAssertTrue(correlation.accepts(requestID: first))
    let second = correlation.begin(command: "printf same")
    XCTAssertNotEqual(first, second)
    XCTAssertTrue(correlation.accepts(requestID: second))
    XCTAssertTrue(correlation.isSettled)
  }

  @MainActor
  func testVisualHierarchyKeepsSecondaryChromeQuietAndCompact() {
    XCTAssertLessThanOrEqual(SeyalDesignTokens.Layout.leftContextWidth, 220)
    XCTAssertLessThanOrEqual(SeyalDesignTokens.Layout.inspectorWidth, 248)
    XCTAssertLessThanOrEqual(SeyalDesignTokens.Layout.blockCornerRadius, 8)

    let block = BlockView(
      presentation: BlockPresentation(
        id: "quiet", command: "pwd", state: .completed, elapsed: "Done",
        timestamp: nil, isSelected: false, actions: []
      ),
      bodyView: NSView()
    )

    XCTAssertEqual(block.layer?.borderWidth ?? -1, CGFloat(0.5), accuracy: CGFloat(0.01))
  }

  @MainActor
  func testBlockTUITakeoverHidesOnlyPresentationChrome() {
    let body = NSView()
    body.translatesAutoresizingMaskIntoConstraints = false
    body.heightAnchor.constraint(equalToConstant: 120).isActive = true
    let block = BlockView(
      presentation: BlockPresentation(
        id: "tui", command: "nvim", state: .running, elapsed: "Live",
        timestamp: nil, isSelected: true, actions: []
      ),
      bodyView: body
    )

    block.setTUITakeover(true)

    XCTAssertTrue(block.subviewsRecursively.contains { $0 === body })
    XCTAssertEqual(block.layer?.borderWidth, 0)
    XCTAssertFalse(body.isHidden)
  }

  @MainActor
  func testComposerModesRespectPaneOwnershipRules() {
    let available = PaneComposerShellView(mode: .available, draft: "git status")
    let busy = PaneComposerShellView(mode: .busy(process: "vite"), draft: "")
    let tui = PaneComposerShellView(mode: .hiddenForTUI, draft: "")

    XCTAssertFalse(available.isHidden)
    XCTAssertFalse(busy.isHidden)
    XCTAssertTrue(tui.isHidden)
    XCTAssertEqual(descendants(of: NSTextView.self, in: available).count, 1)
    XCTAssertTrue(
      descendants(of: NSScrollView.self, in: available)
        .allSatisfy { !$0.hasVerticalScroller }
    )
    XCTAssertTrue(descendants(of: NSTextView.self, in: busy).isEmpty)
  }

  @MainActor
  func testComposerReturnSubmitsInsteadOfInsertingANewline() {
    var submitted: String?
    let composer = PaneComposerShellView(
      mode: .available,
      draft: "echo from composer",
      onSubmit: {
        submitted = $0
        return true
      }
    )
    let editor = try! XCTUnwrap(descendants(of: NSTextView.self, in: composer).first)

    editor.doCommand(by: #selector(NSResponder.insertNewline(_:)))

    XCTAssertEqual(submitted, "echo from composer")
    XCTAssertFalse(editor.string.contains("\n"))
  }

  @MainActor
  func testComposerFieldEditorReturnSubmitsInsteadOfInsertingANewline() {
    var submitted: String?
    let composer = PaneComposerShellView(
      mode: .available,
      draft: "pwd",
      onSubmit: {
        submitted = $0
        return true
      }
    )
    let editor = try! XCTUnwrap(descendants(of: NSTextView.self, in: composer).first)

    editor.doCommand(by: #selector(NSResponder.insertNewlineIgnoringFieldEditor(_:)))

    XCTAssertEqual(submitted, "pwd")
    XCTAssertEqual(editor.string, "")
  }

  @MainActor
  func testComposerPreservesDraftWhenSubmissionIsRejected() {
    let composer = PaneComposerShellView(
      mode: .available,
      draft: "echo while disconnected",
      onSubmit: { _ in false }
    )
    let editor = try! XCTUnwrap(descendants(of: NSTextView.self, in: composer).first)

    editor.doCommand(by: #selector(NSResponder.insertNewline(_:)))

    XCTAssertEqual(editor.string, "echo while disconnected")
  }

  @MainActor
  func testComposerBusyStateDisablesExistingEditorWithoutReplacingPaneView() throws {
    let composer = PaneComposerShellView(mode: .available, draft: "echo busy")
    let editor = try XCTUnwrap(descendants(of: NSTextView.self, in: composer).first)
    composer.setBusy(true, process: "echo busy")
    XCTAssertFalse(editor.isEditable)
    composer.setBusy(false, process: "")
    XCTAssertTrue(editor.isEditable)
  }

  @MainActor
  func testComposerCanRestoreFirstResponderAfterBlockTimelineRebuild() throws {
    let composer = PaneComposerShellView(mode: .available, draft: "")
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 640, height: 120),
      styleMask: .borderless,
      backing: .buffered,
      defer: true
    )
    window.contentView = composer
    window.makeKeyAndOrderFront(nil)
    defer { window.orderOut(nil) }

    let editor = try XCTUnwrap(descendants(of: NSTextView.self, in: composer).first)
    composer.focusEditor()

    XCTAssertTrue(window.firstResponder === editor)
  }

  @MainActor
  func testShellHasExactlyOneVerticalTranscriptScrollOwnerInitially() {
    let shell = SeyalShellPreviewFactory.make(
      frame: NSRect(x: 0, y: 0, width: 1280, height: 800)
    )
    shell.layoutSubtreeIfNeeded()

    let verticalOwners = descendants(of: NSScrollView.self, in: shell)
      .filter(\.hasVerticalScroller)
    XCTAssertEqual(verticalOwners.count, 1)
  }

  @MainActor
  func testProductionShellUsesOneRealSurfaceBeforeFirstRuntimeBlock() throws {
    let shell = SeyalShellProductionFactory.make(
      frame: NSRect(x: 0, y: 0, width: 960, height: 600)
    )
    shell.layoutSubtreeIfNeeded()

    let surfaces = descendants(of: InteractiveMetalSurfaceView.self, in: shell)
    let transcripts = descendants(of: PaneTranscriptView.self, in: shell)
    XCTAssertEqual(surfaces.count, 1)
    XCTAssertEqual(transcripts.count, 1)
    let surface = try XCTUnwrap(surfaces.first)
    let transcript = try XCTUnwrap(transcripts.first)
    XCTAssertTrue(transcript.subviewsRecursively.contains { $0 === surface })
    XCTAssertEqual(descendants(of: BlockView.self, in: shell).count, 0)
    XCTAssertEqual(descendants(of: PaneComposerShellView.self, in: shell).count, 1)
    XCTAssertTrue(descendants(of: TerminalSurfaceHostView.self, in: shell).isEmpty)
  }

  @MainActor
  func testProductionTerminalSurfaceExposesSafeAccessibilityAndInputContract() throws {
    let shell = SeyalShellProductionFactory.make(
      frame: NSRect(x: 0, y: 0, width: 960, height: 600)
    )
    shell.layoutSubtreeIfNeeded()

    let surface = try XCTUnwrap(
      descendants(of: InteractiveMetalSurfaceView.self, in: shell).first
    )

    // The custom Metal surface is the accessibility element. Its metadata is
    // intentionally limited to role/label/state; terminal cells and command
    // text must never be exposed through the accessibility value.
    XCTAssertTrue(surface.isAccessibilityElement())
    XCTAssertEqual(surface.accessibilityRole(), .group)
    XCTAssertEqual(surface.accessibilityRoleDescription(), "Terminal")
    XCTAssertEqual(surface.accessibilityLabel(), "Seyal Terminal")
    XCTAssertTrue(surface.acceptsFirstResponder)
    XCTAssertTrue(InteractiveMetalSurfaceView.pass7InputSelfTest())
  }

  @MainActor
  func testPreviewWorkspaceInventoryMatchesFrozenWorkspaceModel() {
    let state = SeyalShellState.makePreview()

    XCTAssertEqual(
      state.snapshot.workspaces.map(\.name),
      ["Seyal OSS", "Payments Platform", "Infra Operations", "Personal Lab"]
    )
    XCTAssertEqual(state.leftPanelMode, .workspaces)
    XCTAssertEqual(state.snapshot.agents.map(\.name), ["Claude Code", "Codex", "OpenCode"])

    state.setLeftPanelMode(.tabs)
    XCTAssertEqual(state.leftPanelMode, .tabs)
    XCTAssertEqual(state.snapshot.tabs.count, 4)
  }

  @MainActor
  func testPreviewTabSelectionChangesCanonicalUISelection() {
    let state = SeyalShellState.makePreview()

    state.selectTab(id: "tab-agent")

    XCTAssertEqual(state.snapshot.activeTabID, "tab-agent")
    XCTAssertEqual(
      state.snapshot.inspectorRows.first(where: { $0.id == "tab-name" })?.value,
      "Agent Development"
    )
  }

  @MainActor
  func testPreviewNewTabIsRealLocalNavigationState() {
    let state = SeyalShellState.makePreview()
    let originalCount = state.snapshot.tabs.count

    let tab = try! XCTUnwrap(state.createTab())

    XCTAssertEqual(state.snapshot.tabs.count, originalCount + 1)
    XCTAssertEqual(state.snapshot.activeTabID, tab.id)
    XCTAssertEqual(state.activeTab.paneCount, 1)
  }

  @MainActor
  func testProductionNewTabFailsClosedWithoutDistinctExecutionRoute() {
    let state = SeyalShellState.makeProduction()
    let originalTabID = state.snapshot.activeTabID
    let originalTabCount = state.snapshot.tabs.count

    let result = state.createTab()

    XCTAssertNil(result)
    XCTAssertEqual(state.snapshot.tabs.count, originalTabCount)
    XCTAssertEqual(state.snapshot.activeTabID, originalTabID)
    XCTAssertEqual(state.activeTab.paneCount, 1)
    XCTAssertEqual(
      state.lastActionError,
      "Creating tabs is unavailable until a distinct execution route is available."
    )
  }

  @MainActor
  func testPreviewSplitAndCloseArePaneLocal() {
    let state = SeyalShellState.makePreview()
    let firstPaneID = state.activeTab.focusedPaneID
    state.updateDraft("first draft", paneID: firstPaneID)

    let secondPane = try! XCTUnwrap(state.splitPane(id: firstPaneID, axis: .right))
    state.updateDraft("second draft", paneID: secondPane.id)

    XCTAssertEqual(state.activeTab.paneCount, 2)
    XCTAssertEqual(state.activeTab.layoutDescription, "Split right")
    XCTAssertEqual(state.activeTab.panes[firstPaneID]?.draft, "first draft")
    XCTAssertEqual(state.activeTab.panes[secondPane.id]?.draft, "second draft")
    XCTAssertEqual(state.activeTab.focusedPaneID, secondPane.id)

    state.closePane(id: secondPane.id)

    XCTAssertEqual(state.activeTab.paneCount, 1)
    XCTAssertEqual(state.activeTab.focusedPaneID, firstPaneID)
    XCTAssertEqual(state.activeTab.panes[firstPaneID]?.draft, "first draft")
  }

  @MainActor
  func testProductionSplitFailsClosedWithoutDistinctExecutionRoute() {
    let state = SeyalShellState.makeProduction()
    let originalPaneID = state.activeTab.focusedPaneID

    let result = state.splitPane(id: originalPaneID, axis: .right)

    XCTAssertNil(result)
    XCTAssertEqual(state.activeTab.paneCount, 1)
    XCTAssertEqual(state.activeTab.focusedPaneID, originalPaneID)
    XCTAssertEqual(
      state.lastActionError,
      "Splitting panes is unavailable until a distinct execution route is available."
    )
  }

  @MainActor
  func testSubmittingAnotherCommandNeverForgesPriorBlockCompletion() throws {
    let state = SeyalShellState.makePreview()
    let paneID = state.activeTab.focusedPaneID

    XCTAssertNotNil(state.appendCommand("printf first", paneID: paneID))
    XCTAssertNotNil(state.appendCommand("printf second", paneID: paneID))

    let blocks = try XCTUnwrap(state.activeTab.panes[paneID]?.blocks)
    XCTAssertEqual(blocks.map(\.command), ["printf first", "printf second"])
    XCTAssertTrue(
      blocks.allSatisfy { $0.state == .running },
      "only Runtime lifecycle metadata may complete a command Block"
    )
  }

  @MainActor
  func testPreviewInspectorDoesNotFabricateRuntimeTelemetry() {
    let state = SeyalShellState.makePreview()
    let rows = state.snapshot.inspectorRows

    XCTAssertFalse(rows.contains { $0.section == "Runtime" })
    XCTAssertFalse(rows.contains { $0.label == "PID" })
    XCTAssertFalse(rows.contains { $0.label == "CPU" })
    XCTAssertFalse(rows.contains { $0.label == "Memory" })
  }

  @MainActor
  func testInspectorRailFiltersExistingContextOnly() {
    let shell = SeyalShellPreviewFactory.make(
      frame: NSRect(x: 0, y: 0, width: 1280, height: 800)
    )

    shell.debugSetInspectorMode(.tab)
    let tabRows = shell.debugVisibleInspectorRows()
    XCTAssertFalse(tabRows.isEmpty)
    XCTAssertTrue(tabRows.allSatisfy { $0.section == "Tab" })
    XCTAssertTrue(tabRows.contains { $0.id == "tab-name" })
    XCTAssertFalse(tabRows.contains { $0.id == "workspace-name" })

    shell.debugSetInspectorMode(.pane)
    let paneRows = shell.debugVisibleInspectorRows()
    XCTAssertFalse(paneRows.isEmpty)
    XCTAssertTrue(paneRows.allSatisfy { $0.section == "Active Pane" })
    XCTAssertFalse(
      paneRows.contains { $0.label == "PID" || $0.label == "CPU" || $0.label == "Memory" })
  }

  @MainActor
  func testSidebarsCollapseAndCenterPaneReclaimsTheirWidth() throws {
    let shell = SeyalShellPreviewFactory.make(
      frame: NSRect(x: 0, y: 0, width: 1280, height: 800)
    )
    shell.layoutSubtreeIfNeeded()
    let expanded = try XCTUnwrap(shell.debugLayoutContract())

    shell.debugSetSidebarVisibility(left: false, inspector: false)
    shell.layoutSubtreeIfNeeded()
    let collapsed = try XCTUnwrap(shell.debugLayoutContract())

    XCTAssertEqual(collapsed.leftContext.width, 0, accuracy: 1)
    XCTAssertEqual(collapsed.inspector.width, 0, accuracy: 1)
    XCTAssertEqual(collapsed.pane.minX, shell.bounds.minX, accuracy: 1)
    XCTAssertEqual(collapsed.pane.maxX, shell.bounds.maxX, accuracy: 1)
    XCTAssertGreaterThan(
      collapsed.pane.width,
      expanded.pane.width
        + SeyalDesignTokens.Layout.leftContextWidth
        + SeyalDesignTokens.Layout.inspectorWidth
        - 4
    )

    shell.debugSetSidebarVisibility(left: true, inspector: true)
    shell.layoutSubtreeIfNeeded()
    let restored = try XCTUnwrap(shell.debugLayoutContract())
    XCTAssertEqual(
      restored.leftContext.width, SeyalDesignTokens.Layout.leftContextWidth, accuracy: 1)
    XCTAssertEqual(restored.inspector.width, SeyalDesignTokens.Layout.inspectorWidth, accuracy: 1)
    XCTAssertEqual(restored.pane.width, expanded.pane.width, accuracy: 1)
  }

  @MainActor
  func testNativeShortcutMenuUsesMacTerminalConventions() throws {
    let oldMainMenu = NSApp.mainMenu
    let oldWindowsMenu = NSApp.windowsMenu
    defer {
      NSApp.mainMenu = oldMainMenu
      NSApp.windowsMenu = oldWindowsMenu
    }

    let state = SeyalShellState.makePreview()
    let shell = SeyalShellPreviewFactory.make(
      frame: NSRect(x: 0, y: 0, width: 1280, height: 800),
      state: state
    )
    let window = NSWindow(
      contentRect: shell.frame,
      styleMask: [.titled, .closable, .resizable],
      backing: .buffered,
      defer: false
    )
    window.contentView = shell

    let shortcuts = SeyalPreviewShortcutController(window: window, state: state)
    shortcuts.installMenus()

    let workspaceMenu = try XCTUnwrap(NSApp.mainMenu?.item(withTitle: "Workspace")?.submenu)
    let workspace2 = try XCTUnwrap(
      workspaceMenu.items.first { $0.tag == 1 && $0.keyEquivalent == "2" })
    XCTAssertEqual(normalizedModifiers(workspace2), [.command, .control])

    let tabMenu = try XCTUnwrap(NSApp.mainMenu?.item(withTitle: "Tab")?.submenu)
    let tab2 = try XCTUnwrap(tabMenu.items.first { $0.tag == 1 && $0.keyEquivalent == "2" })
    XCTAssertEqual(normalizedModifiers(tab2), [.command])
    let close = try XCTUnwrap(tabMenu.item(withTitle: "Close Focused Pane / Tab / Window"))
    XCTAssertEqual(close.keyEquivalent, "w")
    XCTAssertEqual(normalizedModifiers(close), [.command])
    let nextTab = try XCTUnwrap(tabMenu.item(withTitle: "Next Tab"))
    XCTAssertEqual(nextTab.keyEquivalent, "]")
    XCTAssertEqual(normalizedModifiers(nextTab), [.command, .shift])

    let windowMenu = try XCTUnwrap(NSApp.mainMenu?.item(withTitle: "Window")?.submenu)
    let window2 = try XCTUnwrap(windowMenu.items.first { $0.tag == 1 && $0.keyEquivalent == "2" })
    XCTAssertEqual(normalizedModifiers(window2), [.command, .option])
    let nextWindow = try XCTUnwrap(windowMenu.item(withTitle: "Next Window"))
    XCTAssertEqual(nextWindow.keyEquivalent, "`")
    XCTAssertEqual(normalizedModifiers(nextWindow), [.command])

    let viewMenu = try XCTUnwrap(NSApp.mainMenu?.item(withTitle: "View")?.submenu)
    XCTAssertEqual(
      normalizedModifiers(try XCTUnwrap(viewMenu.item(withTitle: "Toggle Navigation Sidebar"))),
      [.command]
    )
    XCTAssertEqual(
      normalizedModifiers(try XCTUnwrap(viewMenu.item(withTitle: "Toggle Inspector"))),
      [.command, .option]
    )
  }

  @MainActor
  func testCloseShortcutTargetCascadesPaneThenTabThenWindow() {
    let state = SeyalShellState.makePreview()
    let originalTabID = state.activeTab.id
    let secondPane = try! XCTUnwrap(
      state.splitPane(id: state.activeTab.focusedPaneID, axis: .right)
    )

    XCTAssertEqual(
      SeyalPreviewShortcutController.closeTarget(for: state),
      .pane(secondPane.id)
    )

    state.closePane(id: secondPane.id)
    XCTAssertEqual(
      SeyalPreviewShortcutController.closeTarget(for: state),
      .tab(originalTabID)
    )

    while state.activeWorkspace.tabs.count > 1 {
      state.closeTab(id: state.activeTab.id)
    }
    XCTAssertEqual(SeyalPreviewShortcutController.closeTarget(for: state), .window)
    XCTAssertEqual(state.activeTab.paneCount, 1)
  }

  func testCommandHoldHintPolicyRequiresIntentionalCommandOnlyHold() {
    XCTAssertEqual(SeyalShortcutHintPolicy.intentionalHoldDelay, 0.30, accuracy: 0.0001)
    XCTAssertTrue(SeyalShortcutHintPolicy.isCommandOnly([.command]))
    XCTAssertTrue(SeyalShortcutHintPolicy.isCommandOnly([.command, .capsLock]))
    XCTAssertFalse(SeyalShortcutHintPolicy.isCommandOnly([.command, .shift]))
    XCTAssertFalse(SeyalShortcutHintPolicy.isCommandOnly([.command, .option]))
    XCTAssertFalse(SeyalShortcutHintPolicy.isCommandOnly([.control]))
    XCTAssertFalse(SeyalShortcutHintPolicy.isCommandOnly([]))
  }

  @MainActor
  func testShortcutHintOverlayDoesNotChangeShellLayout() throws {
    let shell = SeyalShellPreviewFactory.make(
      frame: NSRect(x: 0, y: 0, width: 1280, height: 800)
    )
    shell.layoutSubtreeIfNeeded()
    let before = try XCTUnwrap(shell.debugLayoutContract())

    let overlay = SeyalShortcutHintOverlay(frame: .zero)
    overlay.present(
      [
        .init(
          targetAccessibilityID: "tab.tab-terminal",
          text: "⌘1",
          id: "tab.tab-terminal"
        ),
        .init(
          targetAccessibilityID: "toggle-left-sidebar",
          text: "⌘0",
          id: "left-sidebar"
        ),
      ], in: shell)
    shell.layoutSubtreeIfNeeded()
    let after = try XCTUnwrap(shell.debugLayoutContract())

    XCTAssertEqual(before.pane, after.pane)
    XCTAssertEqual(before.leftContext, after.leftContext)
    XCTAssertEqual(before.inspector, after.inspector)
    XCTAssertFalse(overlay.isHidden)
    XCTAssertTrue(
      shell.subviewsRecursively.contains {
        $0.accessibilityIdentifier() == "shortcut-hint.tab.tab-terminal"
      })

    overlay.dismiss()
    XCTAssertTrue(overlay.isHidden)
  }

  @MainActor
  func testForcedShortcutHintControllerPresentsSynchronouslyWithoutMonitorTimer() throws {
    let oldMainMenu = NSApp.mainMenu
    let oldWindowsMenu = NSApp.windowsMenu
    defer {
      NSApp.mainMenu = oldMainMenu
      NSApp.windowsMenu = oldWindowsMenu
    }

    let state = SeyalShellState.makePreview()
    let shell = SeyalShellPreviewFactory.make(
      frame: NSRect(x: 0, y: 0, width: 1280, height: 800),
      state: state
    )
    let window = NSWindow(
      contentRect: shell.frame,
      styleMask: [.titled, .closable, .resizable],
      backing: .buffered,
      defer: false
    )
    window.contentView = shell

    let controller = SeyalPreviewShortcutController(window: window, state: state)
    controller.installMenus()
    shell.layoutSubtreeIfNeeded()
    controller.showShortcutHintsForTesting()

    XCTAssertTrue(
      shell.subviewsRecursively.contains {
        $0.accessibilityIdentifier() == "shortcut-hint-overlay"
      })
    XCTAssertTrue(
      shell.subviewsRecursively.contains {
        $0.accessibilityIdentifier() == "shortcut-hint.tab.tab-terminal"
      })
    XCTAssertTrue(
      shell.subviewsRecursively.contains {
        $0.accessibilityIdentifier() == "tab.tab-terminal"
      })
  }

  @MainActor
  func testShortcutRoutingMutatesExistingShellStateWithoutReplacingView() throws {
    let oldMainMenu = NSApp.mainMenu
    let oldWindowsMenu = NSApp.windowsMenu
    defer {
      NSApp.mainMenu = oldMainMenu
      NSApp.windowsMenu = oldWindowsMenu
    }

    let state = SeyalShellState.makePreview()
    let shell = SeyalShellPreviewFactory.make(
      frame: NSRect(x: 0, y: 0, width: 1280, height: 800),
      state: state
    )
    let window = NSWindow(
      contentRect: shell.frame,
      styleMask: [.titled, .closable, .resizable],
      backing: .buffered,
      defer: false
    )
    window.contentView = shell
    let shortcuts = SeyalPreviewShortcutController(window: window, state: state)
    shortcuts.installMenus()

    shell.debugSetSidebarVisibility(left: false, inspector: true)
    let tab2 = NSMenuItem()
    tab2.tag = 1
    shortcuts.selectTabByNumber(tab2)

    XCTAssertTrue(window.contentView === shell)
    XCTAssertEqual(state.snapshot.activeTabID, "tab-agent")
    shell.layoutSubtreeIfNeeded()
    XCTAssertEqual(try XCTUnwrap(shell.debugLayoutContract()).leftContext.width, 0, accuracy: 1)

    let workspace2 = NSMenuItem()
    workspace2.tag = 1
    shortcuts.selectWorkspaceByNumber(workspace2)
    XCTAssertEqual(state.snapshot.activeWorkspaceID, "workspace-payments")
    XCTAssertEqual(state.snapshot.activeTabID, "tab-payments-api")

    shortcuts.nextWorkspace(nil)
    XCTAssertEqual(state.snapshot.activeWorkspaceID, "workspace-infra")
    shortcuts.previousWorkspace(nil)
    XCTAssertEqual(state.snapshot.activeWorkspaceID, "workspace-payments")
  }

  func testShortcutWrappedIndexCyclesBothDirections() {
    XCTAssertEqual(SeyalPreviewShortcutController.wrappedIndex(current: 0, count: 4, offset: -1), 3)
    XCTAssertEqual(SeyalPreviewShortcutController.wrappedIndex(current: 3, count: 4, offset: 1), 0)
    XCTAssertEqual(SeyalPreviewShortcutController.wrappedIndex(current: 1, count: 4, offset: 1), 2)
  }

  @MainActor
  func testFrozenReferenceUsesDenseThreeColumnLayoutWithoutTrailingGap() throws {
    let shell = SeyalShellPreviewFactory.make(
      frame: NSRect(x: 0, y: 0, width: 1280, height: 800)
    )
    shell.layoutSubtreeIfNeeded()
    let contract = try XCTUnwrap(shell.debugLayoutContract())

    XCTAssertEqual(contract.topChrome.width, 1280, accuracy: 1)
    XCTAssertEqual(
      contract.topChrome.height,
      SeyalDesignTokens.Layout.topChromeHeight,
      accuracy: 1
    )
    XCTAssertEqual(
      contract.leftContext.width,
      SeyalDesignTokens.Layout.leftContextWidth,
      accuracy: 1
    )
    XCTAssertEqual(
      contract.inspector.width,
      SeyalDesignTokens.Layout.inspectorWidth,
      accuracy: 1
    )
    XCTAssertEqual(contract.inspector.maxX, shell.bounds.maxX, accuracy: 1)
    XCTAssertEqual(contract.pane.maxX + 1, contract.inspector.minX, accuracy: 1)
    XCTAssertGreaterThan(contract.pane.width, 700)
    XCTAssertGreaterThan(contract.composer.width, 650)
    XCTAssertGreaterThanOrEqual(
      contract.composer.height,
      SeyalDesignTokens.Layout.composerMinHeight - 1
    )
  }

  @MainActor
  func testFrozenReferencePaletteIsDarkAndNotSystemAdaptive() {
    let background = SeyalDesignTokens.Palette.windowBackground.usingColorSpace(.deviceRGB)
    let components = background?.cgColor.components ?? []
    XCTAssertGreaterThanOrEqual(components.count, 3)
    if components.count >= 3 {
      XCTAssertLessThan(components[0], 0.12)
      XCTAssertLessThan(components[1], 0.12)
      XCTAssertLessThan(components[2], 0.12)
    }
  }

  @MainActor
  func testTerminalSurfaceHostContainsPermanentMetalSurface() {
    let host = TerminalSurfaceHostView(frame: NSRect(x: 0, y: 0, width: 640, height: 400))
    XCTAssertTrue(host.subviews.contains { $0 === host.metalSurface })
    XCTAssertFalse(host.subviewsRecursively.contains { $0 is NSTextView })
  }

  @MainActor
  func testShellPreviewRequiresDebugConfigurationAndExplicitOptIn() {
    XCTAssertTrue(
      AppDelegate.shouldUseShellPreview(
        arguments: ["Seyal", "--ui-shell-preview"],
        environment: [:],
        buildConfiguration: "Debug"
      )
    )
    XCTAssertTrue(
      AppDelegate.shouldUseShellPreview(
        arguments: ["Seyal"],
        environment: ["SEYAL_UI_SHELL_PREVIEW": "1"],
        buildConfiguration: "Debug"
      )
    )
    XCTAssertFalse(
      AppDelegate.shouldUseShellPreview(
        arguments: ["Seyal", "--ui-shell-preview"],
        environment: [:],
        buildConfiguration: "Release"
      )
    )
    XCTAssertFalse(
      AppDelegate.shouldUseShellPreview(
        arguments: ["Seyal"],
        environment: [:],
        buildConfiguration: "Debug"
      )
    )
  }

  @MainActor
  private func normalizedModifiers(_ item: NSMenuItem) -> NSEvent.ModifierFlags {
    item.keyEquivalentModifierMask.intersection(.deviceIndependentFlagsMask)
  }

  @MainActor
  private func descendants<T: NSView>(of type: T.Type, in root: NSView) -> [T] {
    root.subviews.flatMap { child -> [T] in
      var matches = child is T ? [child as! T] : []
      matches.append(contentsOf: descendants(of: type, in: child))
      return matches
    }
  }
}

extension NSView {
  fileprivate var subviewsRecursively: [NSView] {
    subviews + subviews.flatMap(\.subviewsRecursively)
  }
}
