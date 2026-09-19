import XCTest

/// XCUI launches a separate Seyal.app that does not load XCTest, so isolation
/// must be explicit `--runtime-dir` launch arguments. One directory per runner
/// process lets terminate/relaunch reconnect to the same fixture Runtime.
enum IsolatedHostedRuntime {
  static let flag = "--runtime-dir"

  static let directory: String = {
    let url = URL(fileURLWithPath: "/tmp").appendingPathComponent(
      "s860ui-\(ProcessInfo.processInfo.processIdentifier)",
      isDirectory: true
    )
    try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    try? FileManager.default.setAttributes(
      [.posixPermissions: 0o700],
      ofItemAtPath: url.path
    )
    return url.path
  }()

  static var launchArguments: [String] { [flag, directory] }
}

extension XCUIApplication {
  @discardableResult
  func launchIsolatedHost() -> XCUIApplication {
    terminate()
    launchArguments += IsolatedHostedRuntime.launchArguments
    launch()
    return self
  }
}
