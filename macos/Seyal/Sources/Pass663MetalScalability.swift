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
/// 3. `display_link_headed` — CAMetalDisplayLink present-proxy latency for
///    1/5/25 visible surfaces (not physical scanout).
/// 4. `one_noisy` — one real Candidate-D stream while N surfaces present.
/// 5. `many_noisy` — synthetic all-visible churn stress (prep tail latency).
/// 6. `plateau_soak` — hide/show lifecycle RSS samples for plateau judgment.
///
/// Environment:
/// - `SEYAL_PASS663_MATRIX` — comma sizes (default `1,5,25,50,125,250`)
/// - `SEYAL_PASS663_VISIBLE` — visible subset sizes (default `1,5,25`)
/// - `SEYAL_PASS663_SOAK_SECONDS` — hide/show cycle budget (default `30`)
/// - `SEYAL_PASS663_SKIP_SYNTHETIC=1` — skip synthetic matrix
/// - `SEYAL_PASS663_SKIP_REAL_PATH=1` — synthetic-only
/// - `SEYAL_PASS663_SKIP_DISPLAY_LINK=1` — skip headed display-link cohort
/// - `SEYAL_PASS663_SKIP_NOISY=1` — skip one/many noisy cohorts
/// - `SEYAL_PASS663_PLATEAU_SOAK=1` — emit plateau_soak row using soak budget
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
    let skipSynthetic =
      ProcessInfo.processInfo.environment["SEYAL_PASS663_SKIP_SYNTHETIC"] == "1"
    let skipRealPath =
      ProcessInfo.processInfo.environment["SEYAL_PASS663_SKIP_REAL_PATH"] == "1"
    let skipDisplayLink =
      ProcessInfo.processInfo.environment["SEYAL_PASS663_SKIP_DISPLAY_LINK"] == "1"
    let skipNoisy = ProcessInfo.processInfo.environment["SEYAL_PASS663_SKIP_NOISY"] == "1"
    let plateauSoak = ProcessInfo.processInfo.environment["SEYAL_PASS663_PLATEAU_SOAK"] == "1"
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
      "pass663_metal_scalability schema=seyal.pass663.metal-scalability.v2 performance_claim=false"
    )
    print(
      "commit=\(commit) device=\(device.name) registry_id=\(device.registryID) os=\(ProcessInfo.processInfo.operatingSystemVersionString) arch=\(archString()) geometry=\(columns)x\(rows) soak_seconds=\(soakSeconds) percentile_method=nearest_rank evidence_class=controlled-host"
    )
    print(
      "note=product_5x10x5_chrome_is_M003_#674; harness measures Metal presentation scaling for N logical surfaces; display_link=CAMetalDisplayLink_present_proxy_not_scanout"
    )

    var rowsOut = [Row]()
    if !skipSynthetic {
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
    }

    if !skipRealPath {
      let realTargets = matrix.filter { $0 <= 25 }
      for panes in realTargets {
        let visible = panes
        do {
          let row = try measureRealPathFanout(
            device: device,
            panes: panes,
            visible: visible,
            soakSeconds: min(soakSeconds, 20),
            noiseMode: .idle
          )
          emit(row)
          rowsOut.append(row)
        } catch {
          print(
            "row panes=\(panes) visible=\(visible) cohort=real_path_fanout status=PLATFORM_LIMITED error=\(error)"
          )
        }
      }
      for panes in matrix where panes > 25 {
        print(
          "row panes=\(panes) visible=0 cohort=distinct_pty_per_pane status=PLATFORM_LIMITED reason=host_pty_ceiling_documented_pass5; not_silently_reduced"
        )
      }
    }

    if !skipDisplayLink {
      for visible in visibleSizes where visible <= 25 {
        do {
          let row = try measureDisplayLinkHeaded(
            device: device,
            panes: visible,
            visible: visible,
            rows: rows,
            columns: columns
          )
          emit(row)
          rowsOut.append(row)
        } catch {
          print(
            "row panes=\(visible) visible=\(visible) cohort=display_link_headed status=PLATFORM_LIMITED error=\(error)"
          )
        }
      }
    }

    if !skipNoisy {
      do {
        let measured = try measureRealPathFanout(
          device: device,
          panes: 5,
          visible: 5,
          soakSeconds: min(max(soakSeconds, 8), 20),
          noiseMode: .oneNoisy
        )
        let row = Row(
          panes: measured.panes,
          visible: measured.visible,
          cohort: "one_noisy",
          status: measured.status,
          rssKib: measured.rssKib,
          dedicatedGpuBytes: measured.dedicatedGpuBytes,
          instanceBytes: measured.instanceBytes,
          atlasResidentBytes: measured.atlasResidentBytes,
          atlasDuplicated: measured.atlasDuplicated,
          rendererCount: measured.rendererCount,
          displayLinkCount: measured.displayLinkCount,
          commandQueueCount: measured.commandQueueCount,
          prepP50Ns: measured.prepP50Ns,
          prepP95Ns: measured.prepP95Ns,
          prepP99Ns: measured.prepP99Ns,
          note: measured.note
        )
        emit(row)
        rowsOut.append(row)
      } catch {
        print("row panes=5 visible=5 cohort=one_noisy status=PLATFORM_LIMITED error=\(error)")
      }

      do {
        let row = try measureManyNoisy(
          device: device,
          panes: 25,
          visible: 25,
          rows: rows,
          columns: columns
        )
        emit(row)
        rowsOut.append(row)
      } catch {
        print("row panes=25 visible=25 cohort=many_noisy status=FAILED error=\(error)")
        return false
      }
    }

    if plateauSoak {
      do {
        let row = try measurePlateauSoak(
          device: device,
          panes: 25,
          visible: 5,
          rows: rows,
          columns: columns,
          soakSeconds: soakSeconds
        )
        emit(row)
        rowsOut.append(row)
      } catch {
        print("row panes=25 visible=5 cohort=plateau_soak status=FAILED error=\(error)")
        return false
      }
    }

    let syntheticOk = rowsOut.contains {
      $0.cohort == "synthetic" && $0.panes == 250 && $0.status == "MEASURED"
    }
    let realOk = rowsOut.contains {
      $0.cohort == "real_path_fanout" && $0.status == "MEASURED" && $0.dedicatedGpuBytes > 0
    }
    let displayOk = skipDisplayLink || rowsOut.contains {
      $0.cohort == "display_link_headed" && $0.status == "MEASURED"
    }
    let plateauOk = !plateauSoak || rowsOut.contains {
      $0.cohort == "plateau_soak" && $0.status == "MEASURED"
    }
    let closes =
      syntheticOk && realOk && displayOk && plateauOk
      && rowsOut.contains { $0.cohort == "one_noisy" || $0.cohort == "many_noisy" }
    print(
      "summary synthetic_250=\(syntheticOk ? "MEASURED" : "MISSING") real_path_fanout_gpu=\(realOk ? "MEASURED" : "MISSING") display_link=\(displayOk ? "OK" : "MISSING") plateau=\(plateauOk ? "OK" : "MISSING") closes_issue_663=\(closes) reason=\(closes ? "harness_topology_AC_met_pending_issue_refine_and_report" : "needs_remaining_cohorts_or_gpu_evidence")"
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

    let soakDeadline = Date().addingTimeInterval(TimeInterval(soakSeconds))
    var cycles = 0
    while Date() < soakDeadline {
      autoreleasepool {
        for (index, renderer) in renderers.enumerated() where index < visible {
          renderer.setVisible(false)
        }
      }
      pumpRunLoop(0.01)
      autoreleasepool {
        for (index, renderer) in renderers.enumerated() where index < visible {
          renderer.setVisible(true)
          try? cells.withUnsafeBufferPointer { buffer in
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
      }
      cycles += 1
      pumpRunLoop(0.01)
    }

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

  private enum NoiseMode {
    case idle
    case oneNoisy
  }

  private static func measureRealPathFanout(
    device: MTLDevice,
    panes: Int,
    visible: Int,
    soakSeconds: Int,
    noiseMode: NoiseMode
  ) throws -> Row {
    let renderers = try (0..<panes).map { _ in try MetalTerminalRenderer(device: device) }
    defer {
      for renderer in renderers {
        renderer.setVisible(false)
      }
    }

    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 640, height: 400),
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
      if isVisible {
        let cellSize = renderer.cellPixelSize(backingScale: 1)
        let layer = makeLayer(
          device: device,
          width: max(8, cellSize.width * 40),
          height: max(8, cellSize.height * 12)
        )
        host.layer?.addSublayer(layer)
        layers.append(layer)
      }
    }

    let frameCounter = Pass663Counter()
    let prepSamples = Pass663SampleBox()
    let bridge = RustDisplayBridge(
      onFrame: { frame in
        MainActor.assumeIsolated {
          guard let native = NativePreparedFrame(bridgeFrame: frame) else { return }
          frameCounter.value += 1
          for (index, renderer) in renderers.enumerated() where index < visible {
            let started = DispatchTime.now().uptimeNanoseconds
            if let _ = try? renderer.update(
              frame: native,
              backingScale: 1,
              forceFullRebuild: false
            ) {
              prepSamples.values.append(
                DispatchTime.now().uptimeNanoseconds &- started
              )
              let cellSize = renderer.cellPixelSize(backingScale: 1)
              _ = renderer.renderOffscreenAndWait(
                width: cellSize.width * native.columns,
                height: cellSize.height * native.rows
              )
            }
          }
        }
      },
      onError: { _ in },
      paneID: "pass663-fanout-\(panes)"
    )

    let coordinator = RuntimeLifecycleRecoveryCoordinator(
      clock: { CACurrentMediaTime() },
      scheduler: { delay, operation in
        let timer = Timer.scheduledTimer(withTimeInterval: delay, repeats: false) { _ in
          MainActor.assumeIsolated { operation() }
        }
        RunLoop.main.add(timer, forMode: .common)
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
    _ = bridge.ensurePreparedSurface()
    bridge.publishCurrentFrame()

    if case .oneNoisy = noiseMode {
      // Drive continuous shell output on the single real PTY while N surfaces
      // fan the same Candidate-D frame (interactive-tail proxy for AC #4).
      _ = bridge.submitCommittedText("while true; do printf 'N'; sleep 0.01; done\n")
    } else {
      _ = bridge.submitCommittedText("printf 'SEYAL-PASS663\\n'; sleep 2\n")
    }

    let deadline = Date().addingTimeInterval(TimeInterval(max(5, soakSeconds)))
    while Date() < deadline {
      pumpRunLoop(0.05)
      if frameCounter.value == 0 {
        bridge.publishCurrentFrame()
      }
    }

    // Measure while surfaces remain visible — hiding first zeroed GPU evidence.
    let dedicated = renderers.reduce(0) { $0 + $1.estimatedDedicatedGPUBytes }
    let instance = renderers.reduce(0) { $0 + Int($1.stats.instanceBytes) }
    let atlas = renderers.reduce(0) { $0 + $1.atlasResidentBytes }
    let prep = percentileSummary(prepSamples.values)
    let rss = physFootprintKib()

    bridge.stop()
    coordinator.cancel()
    pumpRunLoop(0.05)

    guard frameCounter.value > 0 else {
      throw ScalabilityError.noCandidateDFrames
    }

    return Row(
      panes: panes,
      visible: visible,
      cohort: "real_path_fanout",
      status: "MEASURED",
      rssKib: rss,
      dedicatedGpuBytes: dedicated,
      instanceBytes: instance,
      atlasResidentBytes: atlas,
      atlasDuplicated: panes > 1 && atlas > GlyphAtlas.budgetBytes,
      rendererCount: panes,
      displayLinkCount: 0,
      commandQueueCount: panes,
      prepP50Ns: prep.p50,
      prepP95Ns: prep.p95,
      prepP99Ns: prep.p99,
      note:
        "one_runtime_execution_fans_Candidate-D_into_N_Metal_renderers; frames=\(frameCounter.value); noise=\(noiseMode == .oneNoisy ? "continuous_printf" : "idle_probe"); measured_while_visible=true"
    )
  }

  private static func measureDisplayLinkHeaded(
    device: MTLDevice,
    panes: Int,
    visible: Int,
    rows: Int,
    columns: Int
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

    let window = NSWindow(
      contentRect: NSRect(x: 40, y: 40, width: 800, height: 500),
      styleMask: [.titled, .closable],
      backing: .buffered,
      defer: false
    )
    let host = NSView(frame: window.contentRect(forFrameRect: window.frame))
    host.wantsLayer = true
    window.contentView = host
    window.makeKeyAndOrderFront(nil)
    defer { window.orderOut(nil) }

    var drivers = [Pass663DisplayLinkDriver]()
    for (index, renderer) in renderers.enumerated() where index < visible {
      renderer.setVisible(true)
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
      let cellSize = renderer.cellPixelSize(backingScale: 1)
      let layer = makeLayer(
        device: device,
        width: max(8, cellSize.width * min(columns, 40)),
        height: max(8, cellSize.height * min(rows, 12))
      )
      host.layer?.addSublayer(layer)
      let driver = Pass663DisplayLinkDriver(renderer: renderer, layer: layer)
      drivers.append(driver)
    }

    var presentSamples = [UInt64]()
    presentSamples.reserveCapacity(visible * 16)
    for iteration in 0..<16 {
      cells[0].scalar = iteration.isMultiple(of: 2) ? 0x61 : 0x62
      var damage = DamageMask()
      damage.mark(row: 0)
      for (index, renderer) in renderers.enumerated() where index < visible {
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
        if let sample = drivers[index].submitOne(timeout: 2) {
          presentSamples.append(sample)
        }
      }
    }

    for driver in drivers {
      driver.invalidate()
    }

    let dedicated = renderers.reduce(0) { $0 + $1.estimatedDedicatedGPUBytes }
    let instance = renderers.reduce(0) { $0 + Int($1.stats.instanceBytes) }
    let atlas = renderers.reduce(0) { $0 + $1.atlasResidentBytes }
    let present = percentileSummary(presentSamples)
    guard !presentSamples.isEmpty else {
      throw ScalabilityError.displayLinkUnavailable
    }

    return Row(
      panes: panes,
      visible: visible,
      cohort: "display_link_headed",
      status: "MEASURED",
      rssKib: physFootprintKib(),
      dedicatedGpuBytes: dedicated,
      instanceBytes: instance,
      atlasResidentBytes: atlas,
      atlasDuplicated: panes > 1 && atlas > GlyphAtlas.budgetBytes,
      rendererCount: panes,
      displayLinkCount: drivers.count,
      commandQueueCount: panes,
      prepP50Ns: present.p50,
      prepP95Ns: present.p95,
      prepP99Ns: present.p99,
      note:
        "metric=committed_generation_to_CAMetalDisplayLink_present_proxy; samples=\(presentSamples.count); not_physical_scanout"
    )
  }

  private static func measureManyNoisy(
    device: MTLDevice,
    panes: Int,
    visible: Int,
    rows: Int,
    columns: Int
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

    for (index, renderer) in renderers.enumerated() {
      renderer.setVisible(index < visible)
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
    }

    var prepSamples = [UInt64]()
    prepSamples.reserveCapacity(visible * 32)
    for iteration in 0..<32 {
      let row = iteration % rows
      for column in 0..<min(columns, 16) {
        cells[row * columns + column].scalar = UInt32(0x30 + (iteration + column) % 10)
      }
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
        prepSamples.append(DispatchTime.now().uptimeNanoseconds &- started)
      }
      pumpRunLoop(0.001)
    }

    let dedicated = renderers.reduce(0) { $0 + $1.estimatedDedicatedGPUBytes }
    let instance = renderers.reduce(0) { $0 + Int($1.stats.instanceBytes) }
    let atlas = renderers.reduce(0) { $0 + $1.atlasResidentBytes }
    let prep = percentileSummary(prepSamples)

    return Row(
      panes: panes,
      visible: visible,
      cohort: "many_noisy",
      status: "MEASURED",
      rssKib: physFootprintKib(),
      dedicatedGpuBytes: dedicated,
      instanceBytes: instance,
      atlasResidentBytes: atlas,
      atlasDuplicated: panes > 1 && atlas > GlyphAtlas.budgetBytes,
      rendererCount: panes,
      displayLinkCount: 0,
      commandQueueCount: panes,
      prepP50Ns: prep.p50,
      prepP95Ns: prep.p95,
      prepP99Ns: prep.p99,
      note: "synthetic_all_visible_churn; prep_latency_under_many_noisy_stress; samples=\(prepSamples.count)"
    )
  }

  private static func measurePlateauSoak(
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

    for (index, renderer) in renderers.enumerated() {
      renderer.setVisible(index < visible)
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
    }

    var rssSamples = [Int]()
    let soakDeadline = Date().addingTimeInterval(TimeInterval(soakSeconds))
    var cycles = 0
    var nextSample = Date()
    // Sample after surfaces are populated so the baseline excludes cold launch.
    for (index, renderer) in renderers.enumerated() where index < visible {
      renderer.setVisible(true)
    }
    pumpRunLoop(0.05)
    rssSamples.append(physFootprintKib())
    nextSample = Date().addingTimeInterval(max(5, Double(soakSeconds) / 36))

    while Date() < soakDeadline {
      autoreleasepool {
        for (index, renderer) in renderers.enumerated() where index < visible {
          renderer.setVisible(false)
        }
      }
      pumpRunLoop(0.01)
      autoreleasepool {
        for (index, renderer) in renderers.enumerated() where index < visible {
          renderer.setVisible(true)
          try? cells.withUnsafeBufferPointer { buffer in
            _ = try renderer.update(
              frame: NativePreparedFrame(
                cells: buffer,
                generation: UInt64(2_000 + cycles),
                rows: rows,
                columns: columns,
                damage: full
              ),
              backingScale: 1,
              forceFullRebuild: true
            )
          }
        }
      }
      cycles += 1
      pumpRunLoop(0.01)
      if Date() >= nextSample {
        rssSamples.append(physFootprintKib())
        nextSample = Date().addingTimeInterval(max(5, Double(soakSeconds) / 36))
      }
    }

    let last = Array(rssSamples.suffix(max(3, rssSamples.count / 3)))
    let plateau = isPlateau(last)
    let dedicated = renderers.reduce(0) { $0 + $1.estimatedDedicatedGPUBytes }
    let instance = renderers.reduce(0) { $0 + Int($1.stats.instanceBytes) }
    let atlas = renderers.reduce(0) { $0 + $1.atlasResidentBytes }

    return Row(
      panes: panes,
      visible: visible,
      cohort: "plateau_soak",
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
      note:
        "hide_show_cycles=\(cycles); rss_samples=\(rssSamples.count); rss_first=\(rssSamples.first ?? 0); rss_last=\(rssSamples.last ?? 0); plateau=\(plateau); soak_seconds=\(soakSeconds)"
    )
  }

  private static func isPlateau(_ samples: [Int]) -> Bool {
    guard samples.count >= 3 else { return false }
    let mean = Double(samples.reduce(0, +)) / Double(samples.count)
    guard mean > 0 else { return false }
    let variance =
      samples.reduce(0.0) { partial, value in
        let delta = Double(value) - mean
        return partial + delta * delta
      } / Double(samples.count)
    let coeff = (variance.squareRoot()) / mean
    // ≤5% coefficient of variation in the late window → plateau.
    return coeff <= 0.05
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
    if bridge.isConnected { return }
    coordinator.beginEpisode()
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
      if bridge.isConnected { return }
      let stage = coordinator.state.stage
      if stage == .exhausted || stage == .blocked {
        let launch = BundledRuntimeLauncher.consumeLastLaunchError().map(String.init(describing:))
          ?? bridge.lastLaunchError.map(String.init(describing:))
          ?? "none"
        throw ScalabilityError.recoveryFailed(
          stage: String(describing: stage),
          launchError: launch
        )
      }
      pumpRunLoop(0.01)
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

  private enum ScalabilityError: Error, CustomStringConvertible {
    case runtimeAttachTimeout
    case noCandidateDFrames
    case displayLinkUnavailable
    case recoveryFailed(stage: String, launchError: String)

    var description: String {
      switch self {
      case .runtimeAttachTimeout:
        return "runtimeAttachTimeout"
      case .noCandidateDFrames:
        return "noCandidateDFrames"
      case .displayLinkUnavailable:
        return "displayLinkUnavailable"
      case let .recoveryFailed(stage, launchError):
        return "recoveryFailed:\(stage):launch=\(launchError)"
      }
    }
  }
}

private final class Pass663TimerBox: @unchecked Sendable {
  let timer: Timer
  init(timer: Timer) { self.timer = timer }
}

private final class Pass663Counter: @unchecked Sendable {
  var value = 0
}

private final class Pass663SampleBox: @unchecked Sendable {
  var values = [UInt64]()
}

@MainActor
private final class Pass663DisplayLinkDriver: NSObject, @preconcurrency CAMetalDisplayLinkDelegate {
  private let renderer: MetalTerminalRenderer
  private let link: CAMetalDisplayLink
  private var startedAt: UInt64?

  init(renderer: MetalTerminalRenderer, layer: CAMetalLayer) {
    self.renderer = renderer
    link = CAMetalDisplayLink(metalLayer: layer)
    super.init()
    link.delegate = self
    link.isPaused = true
    link.add(to: .main, forMode: .common)
  }

  func submitOne(timeout: TimeInterval) -> UInt64? {
    guard startedAt == nil else { return nil }
    renderer.requestPresent()
    startedAt = DispatchTime.now().uptimeNanoseconds
    link.isPaused = false
    let deadline = Date().addingTimeInterval(timeout)
    var sample: UInt64?
    while sample == nil, Date() < deadline {
      RunLoop.current.run(until: Date().addingTimeInterval(0.001))
      if let startedAt, link.isPaused {
        // Delegate stores via side channel below.
        _ = startedAt
      }
      if let captured = lastSample {
        sample = captured
        lastSample = nil
      }
    }
    if sample == nil {
      startedAt = nil
      link.isPaused = true
    }
    return sample
  }

  private var lastSample: UInt64?

  func metalDisplayLink(
    _ link: CAMetalDisplayLink,
    needsUpdate update: CAMetalDisplayLink.Update
  ) {
    link.isPaused = true
    guard let startedAt,
      renderer.present(drawable: update.drawable)
    else {
      self.startedAt = nil
      return
    }
    lastSample = DispatchTime.now().uptimeNanoseconds &- startedAt
    self.startedAt = nil
  }

  func invalidate() {
    link.delegate = nil
    link.invalidate()
  }
}
