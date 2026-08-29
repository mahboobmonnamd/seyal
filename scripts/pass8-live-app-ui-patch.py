#!/usr/bin/env python3
from pathlib import Path

path = Path("macos/Seyal/Tests/SeyalUITests/SeyalShellUITests.swift")
text = path.read_text()
anchor = '''    func testProductionShellUsesOnePaneOwnedComposerAndMetalSurface() {\n'''
insert = r'''    func testProductionAppExecutesShellCommandThroughExternalRuntime() throws {
        app.terminate()

        var repoRoot = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 {
            repoRoot.deleteLastPathComponent()
        }
        let runtimeURL = repoRoot.appendingPathComponent("target/debug/seyal-runtime")
        let appBinaryURL = repoRoot.appendingPathComponent(
            "target/macos-ui-tests/Build/Products/Debug/Seyal.app/Contents/MacOS/Seyal"
        )
        XCTAssertTrue(FileManager.default.isExecutableFile(atPath: runtimeURL.path))
        XCTAssertTrue(FileManager.default.isExecutableFile(atPath: appBinaryURL.path))

        let markerURL = FileManager.default.temporaryDirectory.appendingPathComponent(
            "seyal-pass8-live-app-\(UUID().uuidString)"
        )
        try? FileManager.default.removeItem(at: markerURL)
        defer { try? FileManager.default.removeItem(at: markerURL) }

        let runtime = Process()
        runtime.executableURL = runtimeURL
        runtime.arguments = ["/bin/zsh"]
        runtime.standardOutput = Pipe()
        runtime.standardError = Pipe()
        try runtime.run()
        defer {
            if runtime.isRunning {
                runtime.terminate()
            }
            runtime.waitUntilExit()
        }

        // Prove discovery/attach is ready through the same production app
        // binary before asking XCUIAutomation to drive the normal window.
        var runtimeReady = false
        for _ in 0..<20 {
            let probe = Process()
            probe.executableURL = appBinaryURL
            probe.arguments = ["--pass8-native-metadata-self-test"]
            probe.standardOutput = Pipe()
            probe.standardError = Pipe()
            try probe.run()
            probe.waitUntilExit()
            if probe.terminationStatus == 0 {
                runtimeReady = true
                break
            }
            if !runtime.isRunning {
                break
            }
            Thread.sleep(forTimeInterval: 0.05)
        }
        XCTAssertTrue(runtimeReady, "external Runtime did not become attachable")

        app = XCUIApplication()
        app.launchArguments = []
        app.launchEnvironment = [:]
        app.launch()

        let window = app.windows["Seyal"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))
        let composer = app.textViews["composer.pane-local"]
        XCTAssertTrue(composer.waitForExistence(timeout: 3))
        composer.click()
        composer.typeText("printf PASS8_BASIC; printf ok > \(markerURL.path)")
        composer.typeKey(.return, modifierFlags: [])

        let deadline = Date().addingTimeInterval(5)
        while !FileManager.default.fileExists(atPath: markerURL.path), Date() < deadline {
            Thread.sleep(forTimeInterval: 0.05)
        }
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: markerURL.path),
            "normal Seyal.app composer input did not reach the external Runtime-owned PTY shell"
        )
    }

'''
if text.count(anchor) != 1:
    raise SystemExit(f"production-shell anchor count={text.count(anchor)}")
path.write_text(text.replace(anchor, insert + anchor, 1))
