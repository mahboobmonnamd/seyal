import Metal
import XCTest

@testable import Seyal

final class Pass9RendererCalibrationTests: XCTestCase {
  @MainActor
  func testDedicatedMetalResourcesReturnToZeroAfterVisibilityLifecycle() throws {
    guard let device = MTLCreateSystemDefaultDevice() else {
      throw XCTSkip("Metal device unavailable on this host")
    }

    let renderer = try MetalTerminalRenderer(device: device)
    renderer.setVisible(false)
    XCTAssertFalse(renderer.hasDedicatedSurfaceResources)
    XCTAssertEqual(renderer.estimatedDedicatedGPUBytes, 0)

    renderer.setVisible(true)
    var damage = DamageMask()
    damage.markAll(rows: 24)
    var cell = SeyalPreparedCell()
    cell.scalar = 0x61
    let cells = [SeyalPreparedCell](repeating: cell, count: 80 * 24)
    let result = try cells.withUnsafeBufferPointer { buffer in
      try renderer.update(
        frame: NativePreparedFrame(
          cells: buffer,
          generation: 1,
          rows: 24,
          columns: 80,
          damage: damage
        ),
        backingScale: 1,
        forceFullRebuild: true
      )
    }

    XCTAssertEqual(result, .updated)
    XCTAssertTrue(renderer.hasDedicatedSurfaceResources)
    XCTAssertGreaterThan(renderer.estimatedDedicatedGPUBytes, 0)

    renderer.setVisible(false)
    XCTAssertFalse(renderer.hasDedicatedSurfaceResources)
    XCTAssertEqual(renderer.estimatedDedicatedGPUBytes, 0)
  }
}
