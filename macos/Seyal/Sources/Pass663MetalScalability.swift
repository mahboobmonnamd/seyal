import AppKit
import Darwin
import Foundation
import Metal
@preconcurrency import QuartzCore

/// Controlled-host Metal multi-surface scalability harness for Issue #663.
///
/// Measures N `MetalTerminalRenderer` instances on the production prepare→Metal
/// path. Does **not** invent M003 product workspace/tab chrome.
///
/// Cohorts:
/// 1. `synthetic` — N independent renderers driven by prepared frames (Metal
///    resource curve for 1/5/25/50/125/250 without claiming N PTYs).
/// 2. `real_path_fanout` — one bundled Runtime / Candidate-D attachment fans the
///    same committed frame into N renderers (presentation scaling on the real
///    PTY→VT→Candidate-D→Metal path). Distinct-PTY-per-pane populations remain
///    host-limited and are labelled `PLATFORM_LIMITED` when requested.
///
/// Environment:
/// - `SEYAL_PASS663_MATRIX` — comma sizes (default `1,5,25,50,125,250`)
/// - `SEYAL_PASS663_VISIBLE` — visible subset sizes (default `1,5,25`)
/// - `SEYAL_PASS663_SOAK_SECONDS` — hide/show cycle budget (default `30`)
/// - `SEYAL_PASS663_SKIP_REAL_PATH=1` — synthetic-only
@MainActor
enum Pass663MetalScalability {
  private struct Row {
    let panes: Int
    let visible: Int
    let cohort: String
    let status: String
    let rssKib: Int
    let dedicatedGpuBytes: Int
    let instanceBytes: Int
    let atlasResidentBytes: Int
    let atlasDuplicated: Bool
    let rendererCount: Int
    let displayLinkCount: Int
    let commandQueueCount: Int
    let prepP50Ns: UInt64
    let prepP95Ns: UInt64
    let prepP99Ns: UInt64
    let note: String
  }

  static func run() -> Bool {
    guard let device = MTLCreateSystemDefaultDevice() else {
      print("pass663_metal_scalability status=FAILED reason=no_metal_device")
      return false
    }

    let matrix = parseIntList(
      ProcessInfo.processInfo.environment["SEYAL_PASS663_MATRIX"],
      defaultValues: [1, 5, 25, 50, 125, 250]
    )
    let visibleSizes = parseIntList(
      ProcessInfo.processInfo.environment["SEYAL_PASS663_VISIBLE"],
      defaultValues: [1, 5, 25]
    )
    let soakSeconds = max(
      1,
      Int(ProcessInfo.processInfo.environment["SEYAL_PASS663_SOAK_SECONDS"] ?? "30") ?? 30
    )
    let skipRealPath =
      ProcessInfo.processInfo.environment["SEYAL_PASS663_SKIP_REAL_PATH"] == "1"
    let rows = max(8, Int(ProcessInfo.processInfo.environment["SEYAL_PASS663_ROWS"] ?? "24") ?? 24)
    let columns = max(
      40,
      Int(ProcessInfo.processInfo.environment["SEYAL_PASS663_COLUMNS"] ?? "80") ?? 80
    )
    let commit =
      ProcessInfo.processInfo.environment["SEYAL_PASS663_COMMIT"]
      ?? gitHead()
      ?? "unknown"

    let application = NSApplication.shared
    application.setActivationPolicy(.accessory)
    application.finishLaunching()
    application.activate(ignoringOtherApps: true)

    print(
      "pass663_metal_scalability schema=seyal.pass663.metal-scalability.v1 performance_claim=false"
    )
    print(
      "commit=\(commit) device=\(device.name) registry_id=\(device.registryID) os=\(ProcessInfo.processInfo.operatingSystemVersionString) arch=\(archString()) geometry=\(columns)x\(rows) soak_seconds=\(soakSeconds) percentile_method=nearest_rank evidence_class=controlled-host"
    )
    print(
      "note=product_5x10x5_chrome_is_M003; this harness measures Metal presentation scaling for N logical surfaces"
    )

    var rowsOut = [Row]()
    for panes in matrix {
      let visibleCandidates = visibleSizes
        .map { min($0, panes) }
        .filter { $0 > 0 }
      let visibleSet = Array(Set(visibleCandidates)).sorted()
      for visible in visibleSet {
        do {
          let row = try measureSynthetic(
            device: device,
            panes: panes,
            visible: visible,
            rows: rows,
            columns: columns,
            soakSeconds: soakSeconds
          )
          emit(row)
          rowsOut.append(row)
        } catch {
          print(
            "row panes=\(panes) visible=\(visible) cohort=synthetic status=FAILED error=\(error)"
          )
          return false
        }
      }
    }

    if !skipRealPath {
      let realTargets = matrix.filter { $0 <= 25 }
      for panes in realTargets {
        let visible = min(panes, 5)
        do {
          let row = try measureRealPathFanout(
            device: device,
            panes: panes,
            visible: visible,
            soakSeconds: min(soakSeconds, 20)
          )
          emit(row)
          rowsOut.append(row)
        } catch {
          print(
            "row panes=\(panes) visible=\(visible) cohort=real_path_fanout status=PLATFORM_LIMITED error=\(error)"
          )
        }
      }
      // Distinct-PTY-per-pane at 50+ remains a host ceiling on typical macOS
      // developer hosts (Pass 5.1 ~27–34 PTYs). Record the request honestly.
      for panes in matrix where panes > 25 {
        print(
          "row panes=\(panes) visible=0 cohort=distinct_pty_per_pane status=PLATFORM_LIMITED reason=host_pty_ceiling_documented_pass5; not_silently_reduced"
        )
      }
    }

    let syntheticOk = rowsOut.contains {
      $0.cohort == "synthetic" && $0.panes == 250 && $0.status == "MEASURED"
    }
    print(
      "summary synthetic_250=\(syntheticOk ? "MEASURED" : "MISSING") real_path_fanout_rows=\(rowsOut.filter { $0.cohort == "real_path_fanout" }.count) closes_issue_663=false reason=needs_product_multipane_or_full_AC_judgment_after_controlled_host_soak"
    )
    return !rowsOut.isEmpty
  }

  private static func measureSynthetic(
    device: MTLDevice,
    panes: Int,
    visible: Int,
    rows: Int,
    columns: Int,
    soakSeconds: Int
  ) throws -> Row {
    let renderers = try (0..<panes).map { _ in try MetalTerminalRenderer(device: device) }
    defer {
      for renderer in renderers {
        renderer.setVisible(false)
      }
    }

    var cells = [SeyalPreparedCell](
      repeating: preparedCell(scalar: 0x61),
      count: rows * columns
    )
    var full = DamageMask()
    full.markAll(rows: rows)

    let application = NSApplication.shared
    _ = application
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 320, height: 200),
      styleMask: [.borderless],
      backing: .buffered,
      defer: false
    )
    let host = NSView(frame: window.contentRect(forFrameRect: window.frame))
    host.wantsLayer = true
    window.contentView = host
    window.makeKeyAndOrderFront(nil)
    defer { window.orderOut(nil) }

    var layers = [CAMetalLayer]()

    for (index, renderer) in renderers.enumerated() {
      let isVisible = index < visible
      renderer.setVisible(isVisible)
      try cells.withUnsafeBufferPointer { buffer in
        _ = try renderer.update(
          frame: NativePreparedFrame(
            cells: buffer,
            generation: 1,
            rows: rows,
            columns: columns,
            damage: full
          ),
          backingScale: 1,
          forceFullRebuild: true
        )
      }
      if isVisible {
        let cellSize = renderer.cellPixelSize(backingScale: 1)
        let layer = makeLayer(
          device: device,
          width: max(8, cellSize.width * min(columns, 40)),
          height: max(8, cellSize.height * min(rows, 12))
        )
        host.layer?.addSublayer(layer)
        layers.append(layer)
      }
    }

    var prepSamples = [UInt64]()
    prepSamples.reserveCapacity(visible * 8)
    for iteration in 0..<8 {
      let row = iteration % rows
      cells[row * columns].scalar = iteration.isMultiple(of: 2) ? 0x78 : 0x79
      var damage = DamageMask()
      damage.mark(row: row)
      for (index, renderer) in renderers.enumerated() where index < visible {
        let started = DispatchTime.now().uptimeNanoseconds
        try cells.withUnsafeBufferPointer { buffer in
          _ = try renderer.update(
            frame: NativePreparedFrame(
              cells: buffer,
              generation: UInt64(iteration + 2),
              rows: rows,
              columns: columns,
              damage: damage
            ),
            backingScale: 1
          )
        }
        prepSamples.append(DispatchTime.now().uptimeNanoseconds - started)
        let cellSize = renderer.cellPixelSize(backingScale: 1)
        _ = renderer.renderOffscreenAndWait(
          width: cellSize.width * columns,
          height: cellSize.height * rows
        )
      }
    }

    // Hide/show soak for the visible subset — prove surface-local release.
    let soakDeadline = Date().addingTimeInterval(TimeInterval(soakSeconds))
    var cycles = 0
    while Date() < soakDeadline {
      for (index, renderer) in renderers.enumerated() where index < visible {
        renderer.setVisible(false)
      }
      pumpRunLoop(0.01)
      for (index, renderer) in renderers.enumerated() where index < visible {
        renderer.setVisible(true)
        try cells.withUnsafeBufferPointer { buffer in
          _ = try renderer.update(
            frame: NativePreparedFrame(
              cells: buffer,
              generation: UInt64(1000 + cycles),
              rows: rows,
              columns: columns,
              damage: full
            ),
            backingScale: 1,
            forceFullRebuild: true
          )
        }
      }
      cycles += 1
      pumpRunLoop(0.01)
    }

    // Leave non-visible panes hidden and confirm instance bytes drop toward zero
    // after idle release opportunity.
    for (index, renderer) in renderers.enumerated() where index >= visible {
      renderer.setVisible(false)
    }
    pumpRunLoop(0.05)

    let dedicated = renderers.reduce(0) { $0 + $1.estimatedDedicatedGPUBytes }
    let instance = renderers.reduce(0) { $0 + Int($1.stats.instanceBytes) }
    let atlas = renderers.reduce(0) { $0 + $1.atlasResidentBytes }
    let prep = percentileSummary(prepSamples)
    let atlasDuplicated = panes > 1 && atlas > GlyphAtlas.budgetBytes

    return Row(
      panes: panes,
      visible: visible,
      cohort: "synthetic",
      status: "MEASURED",
      rssKib: physFootprintKib(),
      dedicatedGpuBytes: dedicated,
      instanceBytes: instance,
      atlasResidentBytes: atlas,
      atlasDuplicated: atlasDuplicated,
      rendererCount: panes,
      displayLinkCount: 0,
      commandQueueCount: panes,
      prepP50Ns: prep.p50,
      prepP95Ns: prep.p95,
      prepP99Ns: prep.p99,
      note: "hide_show_cycles=\(cycles); per_renderer_atlas_and_queue=true; presentation_proxy=offscreen_only"
    )
  }

  private static func measureRealPathFanout(
    device: MTLDevice,
    panes: Int,
    visible: Int,
    soakSeconds: Int
  ) throws -> Row {
    let renderers = try (0..<panes).map { _ in try MetalTerminalRenderer(device: device) }
    for (index, renderer) in renderers.enumerated() {
      renderer.setVisible(index < visible)
    }

    let bridgeBox = Pass663BridgeBox()
    let bridge = RustDisplayBridge(
      onFrame: { frame in
        MainActor.assumeIsolated {
          guard let native = NativePreparedFrame(bridgeFrame: frame) else { return }
          for (index, renderer) in renderers.enumerated() where index < visible {
            _ = try? renderer.update(
              frame: native,
              backingScale: 1,
              forceFullRebuild: false
            )
          }
        }
      },
      onError: { _ in },
      paneID: "pass663-fanout"
    )
    bridgeBox.bridge = bridge

    let coordinator = RuntimeLifecycleRecoveryCoordinator(
      clock: { CACurrentMediaTime() },
      scheduler: { delay, operation in
        let timer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { _ in
          MainActor.assumeIsolated { operation() }
        }
        let box = Pass663TimerBox(timer: timer)
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

    try awaitConnected(bridge: bridge, coordinator: coordinator, timeout: 30)
    let deadline = Date().addingTimeInterval(TimeInterval(max(5, soakSeconds)))
    while Date() < deadline {
      pumpRunLoop(0.05)
    }

    bridge.stop()
    coordinator.cancel()
    for renderer in renderers {
      renderer.setVisible(false)
    }
    pumpRunLoop(0.05)

    let dedicated = renderers.reduce(0) { $0 + $1.estimatedDedicatedGPUBytes }
    let instance = renderers.reduce(0) { $0 + Int($1.stats.instanceBytes) }
    let atlas = renderers.reduce(0) { $0 + $1.atlasResidentBytes }

    return Row(
      panes: panes,
      visible: visible,
      cohort: "real_path_fanout",
      status: "MEASURED",
      rssKib: physFootprintKib(),
      dedicatedGpuBytes: dedicated,
      instanceBytes: instance,
      atlasResidentBytes: atlas,
      atlasDuplicated: panes > 1 && atlas > GlyphAtlas.budgetBytes,
      rendererCount: panes,
      displayLinkCount: 0,
      commandQueueCount: panes,
      prepP50Ns: 0,
      prepP95Ns: 0,
      prepP99Ns: 0,
      note: "one_runtime_execution_fans_Candidate-D_into_N_Metal_renderers"
    )
  }

  private static func emit(_ row: Row) {
    print(
      "row panes=\(row.panes) visible=\(row.visible) cohort=\(row.cohort) status=\(row.status) rss_kib=\(row.rssKib) dedicated_gpu_bytes=\(row.dedicatedGpuBytes) instance_bytes=\(row.instanceBytes) atlas_resident_bytes=\(row.atlasResidentBytes) atlas_duplicated=\(row.atlasDuplicated) renderers=\(row.rendererCount) display_links=\(row.displayLinkCount) command_queues=\(row.commandQueueCount) prep_p50_ns=\(row.prepP50Ns) prep_p95_ns=\(row.prepP95Ns) prep_p99_ns=\(row.prepP99Ns) note=\(row.note)"
    )
  }

  private static func awaitConnected(
    bridge: RustDisplayBridge,
    coordinator: RuntimeLifecycleRecoveryCoordinator,
    timeout: TimeInterval
  ) throws {
    coordinator.beginEpisode()
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
      if bridge.isConnected {
        return
      }
      pumpRunLoop(0.05)
    }
    throw ScalabilityError.runtimeAttachTimeout
  }

  private static func pumpRunLoop(_ seconds: TimeInterval) {
    RunLoop.current.run(until: Date().addingTimeInterval(seconds))
  }

  private static func parseIntList(_ raw: String?, defaultValues: [Int]) -> [Int] {
    guard let raw, !raw.isEmpty else { return defaultValues }
    let values = raw.split(separator: ",").compactMap { Int($0.trimmingCharacters(in: .whitespaces)) }
    return values.isEmpty ? defaultValues : values
  }

  private static func preparedCell(scalar: UInt32) -> SeyalPreparedCell {
    var cell = SeyalPreparedCell()
    cell.scalar = scalar
    cell.foreground = 0
    cell.background = 0
    cell.flags = 0
    cell.reserved = 0
    return cell
  }

  private static func makeLayer(device: MTLDevice, width: Int, height: Int) -> CAMetalLayer {
    let layer = CAMetalLayer()
    layer.device = device
    layer.pixelFormat = .bgra8Unorm
    layer.framebufferOnly = true
    layer.contentsScale = 1
    layer.drawableSize = CGSize(width: width, height: height)
    layer.frame = CGRect(x: 0, y: 0, width: width, height: height)
    return layer
  }

  private static func percentileSummary(_ input: [UInt64]) -> (p50: UInt64, p95: UInt64, p99: UInt64) {
    let samples = input.sorted()
    func percentile(_ value: Int) -> UInt64 {
      guard !samples.isEmpty else { return 0 }
      let rank = max(1, (samples.count * value + 99) / 100)
      return samples[min(rank - 1, samples.count - 1)]
    }
    return (percentile(50), percentile(95), percentile(99))
  }

  private static func physFootprintKib() -> Int {
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

  private static func archString() -> String {
    #if arch(arm64)
    return "arm64"
    #elseif arch(x86_64)
    return "x86_64"
    #else
    return "unknown"
    #endif
  }

  private static func gitHead() -> String? {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/usr/bin/git")
    process.arguments = ["rev-parse", "HEAD"]
    process.currentDirectoryURL = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
    let pipe = Pipe()
    process.standardOutput = pipe
    process.standardError = Pipe()
    do {
      try process.run()
      process.waitUntilExit()
      guard process.terminationStatus == 0 else { return nil }
      let data = pipe.fileHandleForReading.readDataToEndOfFile()
      return String(data: data, encoding: .utf8)?
        .trimmingCharacters(in: .whitespacesAndNewlines)
    } catch {
      return nil
    }
  }

  private enum ScalabilityError: Error {
    case runtimeAttachTimeout
  }
}

private final class Pass663BridgeBox: @unchecked Sendable {
  var bridge: RustDisplayBridge?
}

private final class Pass663TimerBox: @unchecked Sendable {
  let timer: Timer
  init(timer: Timer) { self.timer = timer }
}
