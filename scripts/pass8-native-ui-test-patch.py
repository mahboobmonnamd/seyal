#!/usr/bin/env python3
from pathlib import Path

path = Path("macos/Seyal/Tests/SeyalUITests/SeyalShellUITests.swift")
text = path.read_text()
anchor = '''    override func tearDownWithError() throws {\n        app.terminate()\n        app = nil\n    }\n\n'''
insert = anchor + r'''    func testPass8NativeMetadataSelfTestUsesRealRuntimeAndAppBundle() throws {
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

        let runtime = Process()
        runtime.executableURL = runtimeURL
        runtime.arguments = ["/bin/sh", "-c", "sleep 5"]
        runtime.standardOutput = Pipe()
        runtime.standardError = Pipe()
        try runtime.run()
        defer {
            if runtime.isRunning {
                runtime.terminate()
            }
            runtime.waitUntilExit()
        }

        var passed = false
        var lastOutput = ""
        for _ in 0..<20 {
            let candidate = Process()
            let stdout = Pipe()
            let stderr = Pipe()
            candidate.executableURL = appBinaryURL
            candidate.arguments = ["--pass8-native-metadata-self-test"]
            candidate.standardOutput = stdout
            candidate.standardError = stderr
            try candidate.run()
            candidate.waitUntilExit()

            let output = stdout.fileHandleForReading.readDataToEndOfFile()
                + stderr.fileHandleForReading.readDataToEndOfFile()
            lastOutput = String(decoding: output, as: UTF8.self)
            if candidate.terminationStatus == 0 {
                passed = true
                break
            }
            if !runtime.isRunning {
                break
            }
            Thread.sleep(forTimeInterval: 0.05)
        }

        XCTAssertTrue(
            passed,
            "real Runtime -> Rust client -> Swift Pass 8 metadata self-test failed: \(lastOutput)"
        )
    }

'''
if text.count(anchor) != 1:
    raise SystemExit(f"tearDown anchor count={text.count(anchor)}")
path.write_text(text.replace(anchor, insert, 1))
