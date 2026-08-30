import XCTest

final class Pass9RendererCalibrationUITests: XCTestCase {
  func testPass9RendererCalibrationExecutableModeCompletesAndReportsResourceReturn() throws {
    var repoRoot = URL(fileURLWithPath: #filePath)
    for _ in 0..<5 {
      repoRoot.deleteLastPathComponent()
    }
    let appBinaryURL = repoRoot.appendingPathComponent(
      "target/macos-ui-tests/Build/Products/Debug/Seyal.app/Contents/MacOS/Seyal"
    )
    XCTAssertTrue(FileManager.default.isExecutableFile(atPath: appBinaryURL.path))

    let candidate = Process()
    let stdout = Pipe()
    let stderr = Pipe()
    candidate.executableURL = appBinaryURL
    candidate.arguments = ["--pass9-renderer-calibration"]
    candidate.standardOutput = stdout
    candidate.standardError = stderr
    try candidate.run()
    candidate.waitUntilExit()

    let output = stdout.fileHandleForReading.readDataToEndOfFile()
      + stderr.fileHandleForReading.readDataToEndOfFile()
    let text = String(decoding: output, as: UTF8.self)
    XCTAssertEqual(candidate.terminationStatus, 0, text)
    XCTAssertTrue(text.contains("pass9_renderer_calibration"), text)
    XCTAssertTrue(text.contains("resource_return_every_cycle=true"), text)
    XCTAssertTrue(text.contains("geometry=120x40"), text)
    XCTAssertTrue(text.contains("geometry=80x24"), text)
  }
}
