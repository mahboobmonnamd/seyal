import AppKit
import Darwin
import Metal
@preconcurrency import QuartzCore
import XCTest
@testable import Seyal

final class SeyalNativeGlueTests: XCTestCase {

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

  func testReleaseTrustRulesRejectAdHocHelpers() throws {
    let root = FileManager.default.temporaryDirectory
      .appendingPathComponent(UUID().uuidString, isDirectory: true)
    let bundle = root.appendingPathComponent("Seyal.app", isDirectory: true)
    let macOS = bundle.appendingPathComponent("Contents/MacOS", isDirectory: true)
    let helpers = bundle.appendingPathComponent("Contents/Helpers", isDirectory: true)
    let helper = helpers.appendingPathComponent("seyal-runtime")
    try FileManager.default.createDirectory(at: helpers, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: macOS, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }

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
    try FileManager.default.copyItem(at: URL(fileURLWithPath: "/bin/echo"), to: helper)
    try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: helper.path)

    try adHocSign(helper, identifier: BundledRuntimeLauncher.helperIdentifier)
    try adHocSign(bundle, identifier: "dev.seyal.Seyal.test-fixture-app")

    XCTAssertNoThrow(
      try BundledRuntimeLauncher.evaluateHelperTrust(
        bundleURL: bundle,
        helperURL: helper,
        enforceReleaseRules: false
      )
    )
    XCTAssertThrowsError(
      try BundledRuntimeLauncher.evaluateHelperTrust(
        bundleURL: bundle,
        helperURL: helper,
        enforceReleaseRules: true
      )
    ) {
      XCTAssertEqual($0 as? BundledRuntimeLaunchError, .helperTrustInvalid)
    }
  }

  @MainActor
  func testNativeStaleHandleAdoptionIsRejected() {
    let before = seyal_bridge_pass9_diag_snapshot()
    let bridge = RustDisplayBridge(
      onFrame: { _ in },
      onError: { _ in },
      paneID: "pass9-stale-adopt"
    )
    XCTAssertFalse(bridge.adoptRecoveredHandle(.testOnly(UInt64.max - 17)))
    XCTAssertFalse(bridge.isConnected)
    let after = seyal_bridge_pass9_diag_snapshot()
    XCTAssertEqual(after.connected, before.connected)
    XCTAssertEqual(after.live_handles, before.live_handles)
    XCTAssertEqual(after.pending_handles, before.pending_handles)
  }

  func testExecutionBlockMetadataCABIIsStable() {
    XCTAssertEqual(MemoryLayout<SeyalExecutionBlockMetadata>.size, 40)
    XCTAssertEqual(MemoryLayout<SeyalPreparedCell>.size, 16)
    XCTAssertEqual(MemoryLayout<SeyalPreparedFrame>.size, 88)
    XCTAssertEqual(MemoryLayout<SeyalHistoryRow>.size, 24)
    XCTAssertEqual(MemoryLayout<SeyalBlockRecord>.size, 48)
  }

  @MainActor
  func testNativeProtocolCompatibilityKeepsInteractiveSurfaceMainActorOwned() {
    let surface = InteractiveMetalSurfaceView(
      frame: NSRect(x: 0, y: 0, width: 320, height: 180),
      paneID: "protocol-compat",
      installation: .nativeInteractionProbe
    )
    let displayDelegate: any CAMetalDisplayLinkDelegate = surface
    let textClient: any NSTextInputClient = surface
    XCTAssertNotNil(displayDelegate as AnyObject)
    XCTAssertNotNil(textClient as AnyObject)
    XCTAssertTrue(InteractiveMetalSurfaceView.pass7InputSelfTest())
  }

  @MainActor
  func testDefaultLaunchMenuIsNotAProductShell() {
    XCTAssertEqual(AppDelegate.makeProductionApplicationMenu().items.count, 2)
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
}
