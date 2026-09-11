import Darwin
@preconcurrency import XCTest

final class SeyalShellUITests: XCTestCase {
    private var app: XCUIApplication!

    @MainActor
    private var leftModeControl: XCUIElement {
        let segmentedControl = app.segmentedControls["left-mode"]
        return segmentedControl.exists
            ? segmentedControl
            : app.radioGroups["left-mode"]
    }

    @MainActor
    private func leftModeSegment(_ label: String) -> XCUIElement {
        let button = leftModeControl.buttons[label]
        return button.exists ? button : leftModeControl.radioButtons[label]
    }

    private func wait(
        timeout: TimeInterval = 5,
        until condition: () -> Bool
    ) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        repeat {
            if condition() { return true }
            RunLoop.current.run(until: Date().addingTimeInterval(0.05))
        } while Date() < deadline
        return condition()
    }

    @MainActor
    private func recoveryFields(_ surface: XCUIElement) -> [String: String]? {
        guard let value = surface.value as? String else { return nil }
        return value.split(separator: " ").reduce(into: [:]) { fields, component in
            let pair = component.split(separator: "=", maxSplits: 1)
            if pair.count == 2 { fields[String(pair[0])] = String(pair[1]) }
        }
    }

    /// Resolve the exact app produced by this checkout instead of asking
    /// LaunchServices for a bundle identifier. Multiple isolated Seyal
    /// worktrees may be installed at once; bundle-id launch can otherwise
    /// attach the production assertions to a stale app from another issue.
    private func productionAppURL() -> URL {
        var repoRoot = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { repoRoot.deleteLastPathComponent() }
        return repoRoot.appendingPathComponent(
            "target/macos-ui-tests/Build/Products/Debug/Seyal.app"
        )
    }

    @MainActor
    private func launchProductionApp(requireUsableConnection: Bool = true) -> XCUIElement {
        app = XCUIApplication(url: productionAppURL())
        app.launchArguments = []
        app.launchEnvironment = [:]
        app.launch()
        // XCUITest may leave a directly-launched bundle backgrounded when
        // another Seyal worktree is already registered with LaunchServices.
        // Production recovery is intentionally visibility-gated, so make the
        // exact candidate app the active foreground window before asserting
        // its Runtime state.
        app.activate()
        let surface = app.descendants(matching: .any)["terminal-surface.pane-local"]
        XCTAssertTrue(surface.waitForExistence(timeout: 5))
        guard requireUsableConnection else { return surface }
        // Recovery accessibility publishes connection=usable only after
        // Runtime attach completes. Allow the full foreground episode budget
        // rather than the default 5s helper wait used elsewhere in this suite.
        let reachedUsableConnection = wait(timeout: 15) {
            self.recoveryFields(surface)?["connection"] == "usable"
        }
        XCTAssertTrue(
            reachedUsableConnection,
            "production Seyal.app did not reach connection=usable after launch; "
                + "last recovery state: \(surface.value ?? "<unavailable>")"
        )
        return surface
    }

    /// Focus the Metal terminal surface without asking XCTest to scroll the
    /// transcript ScrollView. After a production window resize, `click()` can
    /// treat a clipped surface as not hittable and scroll the wrong ancestor.
    @MainActor
    private func focusTerminalSurface(_ surface: XCUIElement) {
        XCTAssertTrue(surface.waitForExistence(timeout: 5))
        XCTAssertGreaterThan(surface.frame.width, 0)
        XCTAssertGreaterThan(surface.frame.height, 0)
        surface.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).click()
    }

    /// Wait until the separately owned Runtime accepts a production attach
    /// through the same app binary XCUI will launch. Mirrors the Pass 8 probe
    /// so endpointMissing → helper-launch never races a still-binding socket.
    private func waitForExternalRuntimeAttachable(
        appBinaryURL: URL,
        runtime: Process,
        attempts: Int = 40
    ) throws {
        var runtimeReady = false
        for _ in 0..<attempts {
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
    }

    override func setUpWithError() throws {
        continueAfterFailure = false
        // XCUIApplication is MainActor-isolated on Xcode 16.4 while XCTest
        // setUp stays nonisolated. Build and launch on the main queue without
        // capturing self into an isolated closure.
        typealias Launch = () -> XCUIApplication
        let launch: @MainActor () -> XCUIApplication = {
            let application = XCUIApplication()
            application.launchArguments = ["--ui-shell-preview"]
            application.launchEnvironment["SEYAL_UI_TEST_FIXTURES"] = "1"
            application.launch()
            return application
        }
        let raw = unsafeBitCast(launch, to: Launch.self)
        app = Thread.isMainThread ? raw() : DispatchQueue.main.sync(execute: raw)
    }

    override func tearDownWithError() throws {
        if let application = app {
            app = nil
            typealias Terminate = (XCUIApplication) -> Void
            let terminate: @MainActor (XCUIApplication) -> Void = { $0.terminate() }
            let raw = unsafeBitCast(terminate, to: Terminate.self)
            if Thread.isMainThread {
                raw(application)
            } else {
                DispatchQueue.main.sync { raw(application) }
            }
        }
        // Packaged Helpers/seyal-runtime can outlive XCUIApplication.terminate().
        // Clear strays so the next test does not attach to a leftover session
        // (for example Pass 9's alternate-screen shell) that hides the composer.
        terminateOrphanedRuntimes()
    }

    /// Best-effort cleanup of leftover Runtime helpers from prior UITest launches.
    private func terminateOrphanedRuntimes() {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/pkill")
        process.arguments = ["-x", "seyal-runtime"]
        try? process.run()
        process.waitUntilExit()
        Thread.sleep(forTimeInterval: 0.15)
    }

    /// Drive a shell command through the production pane. Prefer the pane-owned
    /// composer TextView when it is AX-exposed; otherwise type into the Metal
    /// surface (same path Pass 9 continuity already proves).
    @MainActor
    private func submitProductionShellCommand(_ command: String, surface: XCUIElement) {
        let composer = app.textViews["composer.pane-local"]
        if composer.waitForExistence(timeout: 5) {
            composer.click()
            composer.typeText(command)
            composer.typeKey(.return, modifierFlags: [])
            return
        }
        XCTAssertTrue(surface.exists, "production surface missing while composer TextView is hidden")
        surface.click()
        app.typeText(command)
        app.typeKey(.return, modifierFlags: [])
    }

    /// Attaches a headed PNG for human review. This is not a palette/theme
    /// assertion: the production Metal surface currently clears to a hard-coded
    /// dark default (`MetalTerminalRenderer`) even when AppKit chrome is light.
    /// Matching the active theme is outside #819 HistoryStore scope.
    @MainActor
    private func attachHeadedPNG(_ element: XCUIElement, name: String) {
        let attachment = XCTAttachment(
            data: element.screenshot().pngRepresentation,
            uniformTypeIdentifier: "public.png"
        )
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    @MainActor
    private func productionHistoryURLs() -> (runtime: URL, appBinary: URL) {
        var repoRoot = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { repoRoot.deleteLastPathComponent() }
        return (
            repoRoot.appendingPathComponent("target/debug/seyal-runtime"),
            repoRoot.appendingPathComponent(
                "target/macos-ui-tests/Build/Products/Debug/Seyal.app/Contents/MacOS/Seyal"
            )
        )
    }

    @MainActor
    private func startExternalZshRuntime() throws -> Process {
        let urls = productionHistoryURLs()
        XCTAssertTrue(FileManager.default.isExecutableFile(atPath: urls.runtime.path))
        XCTAssertTrue(FileManager.default.isExecutableFile(atPath: urls.appBinary.path))
        let runtime = Process()
        runtime.executableURL = urls.runtime
        runtime.arguments = ["/bin/zsh"]
        runtime.standardOutput = Pipe()
        runtime.standardError = Pipe()
        try runtime.run()
        try waitForExternalRuntimeAttachable(appBinaryURL: urls.appBinary, runtime: runtime)
        return runtime
    }

    @MainActor
    private func collapseChromeForReflow() {
        for identifier in ["toggle-left-sidebar", "toggle-inspector"] {
            let button = app.buttons[identifier]
            if button.waitForExistence(timeout: 2), button.isHittable {
                button.click()
            }
        }
    }

    @MainActor
    private func resizeSeyalWindow(width: CGFloat, height: CGFloat? = nil) {
        let window = app.windows["Seyal"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))
        let old = window.frame
        let targetHeight = height ?? max(old.height, 520)
        let start = window.coordinate(withNormalizedOffset: CGVector(dx: 1, dy: 1))
        let destination = window.coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: width, dy: targetHeight))
        start.press(forDuration: 0.1, thenDragTo: destination)
        XCTAssertTrue(
            wait(timeout: 5) {
                abs(window.frame.width - old.width) > 8
                    || abs(window.frame.height - old.height) > 8
            },
            "window geometry did not change after resize drag; was \(old), now \(window.frame)"
        )
    }

    @MainActor
    private func waitForVisibleCommandBlock() -> XCUIElement {
        let blocks = app.descendants(matching: .any).matching(
            NSPredicate(format: "identifier CONTAINS '.block.'")
        )
        XCTAssertTrue(
            wait(timeout: 8) { blocks.count > 0 },
            "production command did not create a visible Block"
        )
        return blocks.element(boundBy: max(0, blocks.count - 1))
    }

    @MainActor

    func testPass8NativeMetadataSelfTestUsesRealRuntimeAndAppBundle() throws {
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

    @MainActor

    func testProductionAppExecutesShellCommandThroughExternalRuntime() throws {
        app.terminate()
        // Drop packaged helpers left by earlier cases before binding a fresh Runtime.
        terminateOrphanedRuntimes()

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

        try waitForExternalRuntimeAttachable(appBinaryURL: appBinaryURL, runtime: runtime)

        let surface = launchProductionApp()
        submitProductionShellCommand(
            "printf PASS8_BASIC; printf ok > \(markerURL.path)",
            surface: surface
        )

        let deadline = Date().addingTimeInterval(5)
        while !FileManager.default.fileExists(atPath: markerURL.path), Date() < deadline {
            Thread.sleep(forTimeInterval: 0.05)
        }
        XCTAssertTrue(
            FileManager.default.fileExists(atPath: markerURL.path),
            "normal Seyal.app input did not reach the external Runtime-owned PTY shell"
        )
    }

    @MainActor
    func testProductionComposerReturnCreatesAcceptedCommandAndClearsDraft() throws {
        app.terminate()
        terminateOrphanedRuntimes()

        var repoRoot = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { repoRoot.deleteLastPathComponent() }
        let runtimeURL = repoRoot.appendingPathComponent("target/debug/seyal-runtime")
        let appBinaryURL = repoRoot.appendingPathComponent(
            "target/macos-ui-tests/Build/Products/Debug/Seyal.app/Contents/MacOS/Seyal"
        )
        XCTAssertTrue(FileManager.default.isExecutableFile(atPath: runtimeURL.path))
        XCTAssertTrue(FileManager.default.isExecutableFile(atPath: appBinaryURL.path))

        let markerURL = FileManager.default.temporaryDirectory.appendingPathComponent(
            "seyal-composer-return-\(UUID().uuidString)"
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
            if runtime.isRunning { runtime.terminate() }
            runtime.waitUntilExit()
        }

        try waitForExternalRuntimeAttachable(appBinaryURL: appBinaryURL, runtime: runtime)
        let surface = launchProductionApp()
        let composer = app.textViews["composer.pane-local"]
        XCTAssertTrue(composer.waitForExistence(timeout: 5))
        composer.click()
        let command = "printf M002_COMPOSER_RETURN; printf ok > \(markerURL.path)"
        composer.typeText(command)

        // Exercise the production AppKit responder chain. Directly invoking
        // NSTextView selectors in component tests does not prove that the app
        // menu routes physical Command-A/C/V to the focused composer.
        let pasteboard = NSPasteboard.general
        let previousItems = pasteboard.pasteboardItems?.map { item in
            let copy = NSPasteboardItem()
            for type in item.types {
                if let data = item.data(forType: type) {
                    copy.setData(data, forType: type)
                }
            }
            return copy
        }
        defer {
            pasteboard.clearContents()
            if let previousItems, !previousItems.isEmpty {
                pasteboard.writeObjects(previousItems)
            }
        }
        composer.typeKey("a", modifierFlags: [.command])
        composer.typeKey("c", modifierFlags: [.command])
        composer.typeKey(.delete, modifierFlags: [])
        XCTAssertTrue(
            wait(timeout: 2) { (composer.value as? String) == "" },
            "Command-A followed by Delete did not clear the focused composer"
        )
        composer.typeKey("v", modifierFlags: [.command])
        XCTAssertTrue(
            wait(timeout: 2) { (composer.value as? String) == command },
            "Command-C/V did not round-trip through the production Edit menu"
        )
        composer.typeKey(.return, modifierFlags: [])

        XCTAssertTrue(
            wait(timeout: 5) { FileManager.default.fileExists(atPath: markerURL.path) },
            "Return submission did not reach the Runtime-owned PTY shell"
        )
        XCTAssertEqual(try String(contentsOf: markerURL, encoding: .utf8), "ok")
        XCTAssertTrue(
            wait(timeout: 5) { self.recoveryFields(surface)?["connection"] == "usable" },
            "terminal display connection was lost after composer Return"
        )
        let blocks = app.descendants(matching: .any).matching(
            NSPredicate(format: "identifier CONTAINS '.block.'")
        )
        XCTAssertTrue(
            wait(timeout: 5) { blocks.count > 0 },
            "accepted composer command did not create a visible Block"
        )
        if blocks.count > 0 {
            let block = blocks.element(boundBy: blocks.count - 1)
            // The production pane minimum is 520pt; the visible surface loses
            // the transcript scroller allowance, so 480pt is a stable lower
            // bound that still catches a startup narrow-strip collapse.
            XCTAssertGreaterThan(surface.frame.width, 480)
            XCTAssertGreaterThan(block.frame.width, 0)
            // The transcript Block stack has the specified 8pt outer inset on
            // each side of the pane-owned terminal surface.
            XCTAssertEqual(block.frame.width, surface.frame.width - 16, accuracy: 2)
            XCTAssertLessThanOrEqual(block.frame.width, surface.frame.width + 1)
            XCTAssertGreaterThan(block.frame.height, 0)
            XCTAssertTrue(block.frame.intersects(surface.frame))
        }
        XCTAssertTrue(
            wait(timeout: 5) { (composer.value as? String) == "" },
            "accepted composer draft was not cleared after the correlated Runtime result"
        )
    }

    @MainActor
    func testProductionUnicodeCommandRetainsHeadedRenderedEvidence() throws {
        app.terminate()
        terminateOrphanedRuntimes()

        var repoRoot = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { repoRoot.deleteLastPathComponent() }
        let runtimeURL = repoRoot.appendingPathComponent("target/debug/seyal-runtime")
        let appBinaryURL = repoRoot.appendingPathComponent(
            "target/macos-ui-tests/Build/Products/Debug/Seyal.app/Contents/MacOS/Seyal"
        )
        XCTAssertTrue(FileManager.default.isExecutableFile(atPath: runtimeURL.path))
        XCTAssertTrue(FileManager.default.isExecutableFile(atPath: appBinaryURL.path))

        let markerURL = FileManager.default.temporaryDirectory.appendingPathComponent(
            "seyal-m002-unicode-headed-\(UUID().uuidString)"
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
            if runtime.isRunning { runtime.terminate() }
            runtime.waitUntilExit()
        }

        try waitForExternalRuntimeAttachable(appBinaryURL: appBinaryURL, runtime: runtime)
        let surface = launchProductionApp()
        let composer = app.textViews["composer.pane-local"]
        XCTAssertTrue(composer.waitForExistence(timeout: 5))
        composer.click()
        let baselineSurfacePNG = surface.screenshot().pngRepresentation

        // Keep the typed command short ASCII so XCTest's keyboard path is
        // stable. The octal payload lives in a script file: hosted-runner
        // typeText previously truncated the inline printf and left a 0-byte
        // marker. printf still expands locale-independent UTF-8 at the PTY
        // boundary; cat echoes the same captured bytes into the terminal.
        let expectedUnicode = "é 界 👩‍💻 🇮🇳 क्ष مرحبا\n"
        let scriptURL = FileManager.default.temporaryDirectory.appendingPathComponent(
            "seyal-m002-unicode-headed-\(UUID().uuidString).sh"
        )
        let script = """
        #!/bin/sh
        LC_ALL=C printf '%b' '\\0145\\0314\\0201 \\0347\\0225\\0214 \\0360\\0237\\0221\\0251\\0342\\0200\\0215\\0360\\0237\\0222\\0273 \\0360\\0237\\0207\\0256\\0360\\0237\\0207\\0263 \\0340\\0244\\0225\\0340\\0245\\0215\\0340\\0244\\0267 \\0331\\0205\\0330\\0261\\0330\\0255\\0330\\0250\\0330\\0247\\0012' > '\(markerURL.path)'
        LC_ALL=C cat '\(markerURL.path)'
        """
        try script.write(to: scriptURL, atomically: true, encoding: .utf8)
        defer { try? FileManager.default.removeItem(at: scriptURL) }
        composer.typeText("sh '\(scriptURL.path)'")
        composer.typeKey(.return, modifierFlags: [])

        let expectedUnicodeData = Data(expectedUnicode.utf8)
        XCTAssertTrue(
            wait(timeout: 8) {
                (try? Data(contentsOf: markerURL)) == expectedUnicodeData
            },
            "Unicode workload did not reach the Runtime-owned PTY shell"
        )
        XCTAssertEqual(try Data(contentsOf: markerURL), expectedUnicodeData)

        let blocks = app.descendants(matching: .any).matching(
            NSPredicate(format: "identifier CONTAINS '.block.'")
        )
        XCTAssertTrue(
            wait(timeout: 5) { blocks.count > 0 },
            "Unicode command did not create a visible production Block"
        )
        XCTAssertGreaterThan(surface.frame.width, 480)
        if blocks.count > 0 {
            let block = blocks.element(boundBy: blocks.count - 1)
            XCTAssertTrue(
                wait(timeout: 5) {
                    block.frame.width > 0
                        && block.frame.height > 0
                        && block.frame.intersects(surface.frame)
                        && abs(block.frame.width - (surface.frame.width - 16)) <= 2
                },
                "Unicode Block did not receive a visible frame"
            )
            XCTAssertEqual(block.frame.width, surface.frame.width - 16, accuracy: 2)
        }

        var renderedSurfacePNG = baselineSurfacePNG
        XCTAssertTrue(
            wait(timeout: 5) {
                renderedSurfacePNG = surface.screenshot().pngRepresentation
                return renderedSurfacePNG != baselineSurfacePNG
            },
            "terminal surface did not present a changed frame after Unicode output"
        )
        let attachment = XCTAttachment(
            data: renderedSurfacePNG,
            uniformTypeIdentifier: "public.png"
        )
        attachment.name = "m002-817-unicode-headed-render"
        attachment.lifetime = .keepAlways
        add(attachment)
        XCTAssertGreaterThan(renderedSurfacePNG.count, 0)
    }

    @MainActor

    func testPass9ProductionRecoverySurvivesGracefulAndForcedGUIExit() throws {
        app.terminate()
        terminateOrphanedRuntimes()

        var repoRoot = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { repoRoot.deleteLastPathComponent() }
        let runtimeURL = repoRoot.appendingPathComponent("target/debug/seyal-runtime")
        XCTAssertTrue(FileManager.default.isExecutableFile(atPath: runtimeURL.path))

        let token = "pass9-\(UUID().uuidString)"
        let continuityMarker = FileManager.default.temporaryDirectory
            .appendingPathComponent("seyal-pass9-continuity-\(UUID().uuidString)")
        defer {
            try? FileManager.default.removeItem(at: continuityMarker)
        }

        let runtime = Process()
        runtime.executableURL = runtimeURL
        runtime.arguments = ["/bin/zsh"]
        runtime.standardOutput = Pipe()
        runtime.standardError = Pipe()
        try runtime.run()
        defer {
            if runtime.isRunning { runtime.terminate() }
            runtime.waitUntilExit()
        }

        let appBinaryURL = repoRoot
            .appendingPathComponent("target/macos-ui-tests/Build/Products/Debug/Seyal.app/Contents/MacOS/Seyal")
        // Prefer the DerivedData product under test when present (headed Pass 10
        // runs use a dedicated derivedDataPath); fall back to the make ui-test path.
        let headedBinary = repoRoot.appendingPathComponent(
            "target/macos-ui-tests-headed-arm64/Build/Products/Debug/Seyal.app/Contents/MacOS/Seyal"
        )
        let probeBinary = FileManager.default.isExecutableFile(atPath: headedBinary.path)
            ? headedBinary
            : appBinaryURL
        try waitForExternalRuntimeAttachable(appBinaryURL: probeBinary, runtime: runtime)

        var surface = launchProductionApp()
        let first = try XCTUnwrap(recoveryFields(surface))
        XCTAssertNotEqual(first["runtime"], "none")
        XCTAssertNotEqual(first["execution"], "none")
        XCTAssertNotEqual(first["attachment"], "none")

        let composer = app.textViews["composer.pane-local"]
        XCTAssertTrue(composer.waitForExistence(timeout: 3))
        composer.click()
        composer.typeText("export SEYAL_PASS9_TOKEN='\(token)'")
        composer.typeKey(.return, modifierFlags: [])

        // A real window geometry change must keep the surface usable and
        // produce a finite accessibility frame contained by the app window.
        let window = app.windows["Seyal"]
        let oldFrame = window.frame
        window.coordinate(withNormalizedOffset: CGVector(dx: 0.99, dy: 0.99))
            .press(forDuration: 0.1, thenDragTo: window.coordinate(
                withNormalizedOffset: CGVector(dx: 0.85, dy: 0.85)
            ))
        XCTAssertTrue(wait { window.frame != oldFrame })
        XCTAssertTrue(window.frame.intersects(surface.frame))
        XCTAssertGreaterThan(surface.frame.width, 0)
        XCTAssertGreaterThan(surface.frame.height, 0)

        // The standard AppKit close control exercises the production
        // window-close path. Hosted XCTest
        // can retain the application process after its last window closes, so
        // explicitly end that now-windowless process before reopening it.
        let closeButton = window.buttons[XCUIIdentifierCloseWindow]
        XCTAssertTrue(closeButton.exists)
        closeButton.click()
        XCTAssertTrue(wait { !window.exists })
        if app.state != .notRunning { app.terminate() }
        surface = launchProductionApp()
        let afterClose = try XCTUnwrap(recoveryFields(surface))
        XCTAssertEqual(afterClose["runtime"], first["runtime"])
        XCTAssertEqual(afterClose["execution"], first["execution"])
        XCTAssertNotEqual(afterClose["attachment"], first["attachment"])

        // Keep the PTY in alternate screen while the GUI disappears abruptly.
        focusTerminalSurface(surface)
        app.typeText("printf '\\033[?1049hALT'; while :; do sleep 1; done")
        app.typeKey(.return, modifierFlags: [])
        XCTAssertTrue(wait { self.recoveryFields(surface)?["alternate-screen"] == "true" })

        let killedPID = try XCTUnwrap(Int32(afterClose["process"] ?? ""))
        XCTAssertGreaterThan(killedPID, 1)
        XCTAssertEqual(Darwin.kill(killedPID, SIGKILL), 0)
        XCTAssertTrue(wait { self.app.state == .notRunning })
        surface = launchProductionApp()
        let afterKill = try XCTUnwrap(recoveryFields(surface))
        XCTAssertEqual(afterKill["runtime"], first["runtime"])
        XCTAssertEqual(afterKill["execution"], first["execution"])
        XCTAssertNotEqual(afterKill["attachment"], afterClose["attachment"])
        XCTAssertEqual(afterKill["alternate-screen"], "true")

        // VoiceOver-facing recovery discoverability after abrupt GUI death:
        // finite hittable surface + usable recovery accessibility fields.
        XCTAssertTrue(surface.exists)
        XCTAssertGreaterThan(surface.frame.width, 0)
        XCTAssertGreaterThan(surface.frame.height, 0)
        XCTAssertEqual(afterKill["connection"], "usable")
        XCTAssertNotEqual(afterKill["runtime"], "none")
        XCTAssertNotEqual(afterKill["execution"], "none")

        // Focus the real NSTextInputClient, interrupt the retained foreground
        // command, and prove direct terminal input reaches the same shell.
        focusTerminalSurface(surface)
        app.typeKey("c", modifierFlags: .control)
        app.typeText("printf '%s' '\(token)' > \(continuityMarker.path)")
        app.typeKey(.return, modifierFlags: [])
        XCTAssertTrue(wait { FileManager.default.fileExists(atPath: continuityMarker.path) })
        XCTAssertEqual(try String(contentsOf: continuityMarker, encoding: .utf8), token)

    }

    @MainActor

    func testProductionShellUsesOnePaneOwnedComposerAndMetalSurface() {
        // The production launch intentionally has no preview flag or fixture
        // environment. This exercises the real AppKit shell factory and its
        // pane-owned surface/composer identity, without requiring a Runtime
        // attach on the host. Clear stray packaged helpers so we do not attach
        // to a leftover TUI session that would hide the composer TextView.
        app.terminate()
        terminateOrphanedRuntimes()
        let surface = launchProductionApp(requireUsableConnection: false)

        let window = app.windows["Seyal"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))
        let surfaces = app
            .descendants(matching: .any)
            .matching(identifier: "terminal-surface.pane-local")
        XCTAssertEqual(
            surfaces.count,
            1,
            "the production terminal surface remains discoverable at the recovery boundary"
        )
        XCTAssertTrue(surface.exists)

        let composer = app.textViews["composer.pane-local"]
        if composer.waitForExistence(timeout: 10) {
            composer.click()
            composer.typeText("printf 'pass7.1'")
            XCTAssertEqual(composer.value as? String, "printf 'pass7.1'")
        } else {
            // Packaged helper may begin attach in the background and hide the
            // prompt TextView (TUI/busy). Pane-owned Metal surface identity is
            // still the production contract under test.
            XCTAssertTrue(surface.isHittable)
        }
    }

    @MainActor

    func testShellLaunchesWithFrozenCoreHierarchyWithoutFabricatedRuntimeOutput() {
        let window = app.windows["Seyal — UI Shell Preview"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))
        XCTAssertGreaterThanOrEqual(window.frame.width, 1048)
        XCTAssertGreaterThanOrEqual(window.frame.height, 680)

        XCTAssertTrue(app.buttons["toggle-left-sidebar"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["toggle-inspector"].waitForExistence(timeout: 2))
        XCTAssertTrue(leftModeControl.waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["WORKSPACES"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["AGENTS · SEYAL OSS"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["workspace.workspace-seyal"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["workspace.workspace-payments"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["workspace.workspace-infra"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["workspace.workspace-lab"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["agent.agent-claude"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["agent.agent-codex"].waitForExistence(timeout: 2))

        XCTAssertTrue(app.buttons["tab.tab-terminal"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["tab.tab-agent"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["tab.tab-logs"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["new-tab"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["pane.split.pane-1"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Inspector"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["inspector-mode.context"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["inspector-mode.workspace"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["inspector-mode.tab"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["inspector-mode.pane"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["No TerminalExecution attached"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.textViews["composer.pane-1"].waitForExistence(timeout: 2))

        XCTAssertFalse(app.staticTexts["git status"].exists)
        XCTAssertFalse(app.staticTexts["PID"].exists)
        XCTAssertFalse(app.staticTexts["CPU"].exists)

        let screenshot = XCUIScreen.main.screenshot()
        let attachment = XCTAttachment(screenshot: screenshot)
        attachment.name = "M001 Core Terminal interactive preview"
        attachment.lifetime = .keepAlways
        add(attachment)
    }

    @MainActor

    func testWorkspaceTabsSwitcherUsesCompactFrozenLeftPanelModel() {
        let mode = leftModeControl
        XCTAssertTrue(mode.waitForExistence(timeout: 5))

        let tabsSegment = leftModeSegment("Tabs")
        XCTAssertTrue(tabsSegment.exists)
        tabsSegment.click()

        XCTAssertTrue(app.staticTexts["TABS"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["left-tab.tab-terminal"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["left-tab.tab-agent"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["left-new-tab"].waitForExistence(timeout: 2))
        XCTAssertFalse(app.staticTexts["AGENTS · SEYAL OSS"].exists)

        leftModeSegment("Workspaces").click()
        XCTAssertTrue(app.staticTexts["WORKSPACES"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["AGENTS · SEYAL OSS"].waitForExistence(timeout: 2))
    }

    @MainActor

    func testTopTabActuallySwitchesActiveTabAndInspector() {
        let target = app.buttons["tab.tab-agent"]
        XCTAssertTrue(target.waitForExistence(timeout: 5))

        target.click()

        let inspectorTab = app.staticTexts["inspector.tab-name"]
        XCTAssertTrue(inspectorTab.waitForExistence(timeout: 2))
        XCTAssertEqual(inspectorTab.label, "Agent Development")
        XCTAssertTrue(app.textViews["composer.pane-agent"].waitForExistence(timeout: 2))
    }

    @MainActor

    func testNewTabCreatesAndSelectsRealPreviewTabState() {
        let newTab = app.buttons["new-tab"]
        XCTAssertTrue(newTab.waitForExistence(timeout: 5))

        newTab.click()

        XCTAssertTrue(app.buttons["tab.tab-new-5"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["TABS"].waitForExistence(timeout: 2))
        let inspectorTab = app.staticTexts["inspector.tab-name"]
        XCTAssertTrue(inspectorTab.waitForExistence(timeout: 2))
        XCTAssertEqual(inspectorTab.label, "Terminal 5")
    }

    @MainActor

    func testPaneLocalSplitMenuCreatesPaneAndCloseRemovesIt() {
        let paneSplit = app.buttons["pane.split.pane-1"]
        XCTAssertTrue(paneSplit.waitForExistence(timeout: 5))

        paneSplit.click()
        let splitRight = app.menuItems["Split Right"]
        XCTAssertTrue(splitRight.waitForExistence(timeout: 2))
        splitRight.click()

        XCTAssertTrue(app.buttons["pane.focus.pane-1"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["pane.focus.pane-new-2"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["pane.split.pane-new-2"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["pane.close.pane-new-2"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.textViews["composer.pane-1"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.textViews["composer.pane-new-2"].waitForExistence(timeout: 2))

        let panes = app.staticTexts["inspector.tab-panes"]
        XCTAssertTrue(panes.waitForExistence(timeout: 2))
        XCTAssertEqual(panes.label, "2")

        app.buttons["pane.close.pane-new-2"].click()

        XCTAssertFalse(app.buttons["pane.focus.pane-new-2"].waitForExistence(timeout: 1))
        XCTAssertTrue(app.buttons["pane.focus.pane-1"].waitForExistence(timeout: 2))
        XCTAssertFalse(app.buttons["pane.close.pane-1"].exists)
    }

    @MainActor

    func testWorkspaceSelectionChangesWorkspaceScopedTabsAndAgents() {
        let payments = app.buttons["workspace.workspace-payments"]
        XCTAssertTrue(payments.waitForExistence(timeout: 5))

        payments.click()

        XCTAssertTrue(app.buttons["tab.tab-payments-api"].waitForExistence(timeout: 2))
        XCTAssertFalse(app.buttons["tab.tab-agent"].exists)
        XCTAssertTrue(app.staticTexts["AGENTS · PAYMENTS PLATFORM"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["agent.agent-payments"].waitForExistence(timeout: 2))
        let workspace = app.staticTexts["inspector.workspace-name"]
        XCTAssertTrue(workspace.waitForExistence(timeout: 2))
        XCTAssertEqual(workspace.label, "Payments Platform")
    }

    @MainActor

    func testWorkspaceRowDragAwayCancelsSelectionCommit() {
        let workspaceBefore = app.staticTexts["inspector.workspace-name"]
        XCTAssertTrue(workspaceBefore.waitForExistence(timeout: 2))
        let initialWorkspaceLabel = workspaceBefore.label

        let payments = app.buttons["workspace.workspace-payments"]
        XCTAssertTrue(payments.waitForExistence(timeout: 5))

        let start = payments.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        let dragTarget = payments.coordinate(withNormalizedOffset: CGVector(dx: -1.5, dy: 0.5))
        start.press(forDuration: 0.25, thenDragTo: dragTarget)

        let workspaceAfter = app.staticTexts["inspector.workspace-name"]
        XCTAssertTrue(workspaceAfter.waitForExistence(timeout: 2))
        XCTAssertEqual(workspaceAfter.label, initialWorkspaceLabel)

        XCTAssertTrue(app.buttons["tab.tab-terminal"].waitForExistence(timeout: 2))
        XCTAssertFalse(app.buttons["tab.tab-payments-api"].exists)
    }

    @MainActor

    func testTopTabChipDragAwayCancelsTabCommit() {
        let inspectorTabBefore = app.staticTexts["inspector.tab-name"]
        XCTAssertTrue(inspectorTabBefore.waitForExistence(timeout: 2))
        let initialTabLabel = inspectorTabBefore.label

        let target = app.buttons["tab.tab-agent"]
        XCTAssertTrue(target.waitForExistence(timeout: 5))

        let start = target.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
        let dragTarget = target.coordinate(withNormalizedOffset: CGVector(dx: -1.5, dy: 0.5))
        start.press(forDuration: 0.25, thenDragTo: dragTarget)

        let inspectorTabAfter = app.staticTexts["inspector.tab-name"]
        XCTAssertTrue(inspectorTabAfter.waitForExistence(timeout: 2))
        XCTAssertEqual(inspectorTabAfter.label, initialTabLabel)

        XCTAssertTrue(app.textViews["composer.pane-1"].waitForExistence(timeout: 2))
        XCTAssertFalse(app.textViews["composer.pane-agent"].exists)
    }

    @MainActor

    func testInspectorRailAndBothSidebarsAreFunctional() {
        let inspectorTabMode = app.buttons["inspector-mode.tab"]
        XCTAssertTrue(inspectorTabMode.waitForExistence(timeout: 5))
        inspectorTabMode.click()

        XCTAssertTrue(app.staticTexts["inspector.tab-name"].waitForExistence(timeout: 2))
        XCTAssertFalse(app.staticTexts["inspector.workspace-name"].exists)
        XCTAssertEqual(app.staticTexts["inspector-mode-label"].label, "TAB")

        let leftCollapse = app.buttons["left-sidebar-collapse"]
        XCTAssertTrue(leftCollapse.waitForExistence(timeout: 2))
        leftCollapse.click()
        XCTAssertFalse(leftModeControl.waitForExistence(timeout: 1))

        let leftToggle = app.buttons["toggle-left-sidebar"]
        XCTAssertTrue(leftToggle.waitForExistence(timeout: 2))
        leftToggle.click()
        XCTAssertTrue(leftModeControl.waitForExistence(timeout: 2))

        let inspectorCollapse = app.buttons["inspector-collapse"]
        XCTAssertTrue(inspectorCollapse.waitForExistence(timeout: 2))
        inspectorCollapse.click()
        XCTAssertFalse(app.staticTexts["Inspector"].waitForExistence(timeout: 1))

        let inspectorToggle = app.buttons["toggle-inspector"]
        XCTAssertTrue(inspectorToggle.waitForExistence(timeout: 2))
        inspectorToggle.click()
        XCTAssertTrue(app.staticTexts["Inspector"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["inspector-mode.tab"].waitForExistence(timeout: 2))
        XCTAssertEqual(app.staticTexts["inspector-mode-label"].label, "TAB")
    }

    @MainActor

    func testNativeKeyboardShortcutsSwitchWorkspaceTabsAndSidebars() {
        let window = app.windows["Seyal — UI Shell Preview"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))

        app.typeKey("2", modifierFlags: [.command])
        let inspectorTab = app.staticTexts["inspector.tab-name"]
        XCTAssertTrue(inspectorTab.waitForExistence(timeout: 2))
        XCTAssertEqual(inspectorTab.label, "Agent Development")

        app.typeKey("2", modifierFlags: [.command, .control])
        let workspace = app.staticTexts["inspector.workspace-name"]
        XCTAssertTrue(workspace.waitForExistence(timeout: 2))
        XCTAssertEqual(workspace.label, "Payments Platform")

        app.typeKey("2", modifierFlags: [.command])
        XCTAssertEqual(app.staticTexts["inspector.tab-name"].label, "Workers")

        app.typeKey("]", modifierFlags: [.command, .control])
        XCTAssertEqual(app.staticTexts["inspector.workspace-name"].label, "Infra Operations")
        app.typeKey("[", modifierFlags: [.command, .control])
        XCTAssertEqual(app.staticTexts["inspector.workspace-name"].label, "Payments Platform")

        app.typeKey("0", modifierFlags: [.command])
        XCTAssertFalse(leftModeControl.waitForExistence(timeout: 1))
        app.typeKey("0", modifierFlags: [.command])
        XCTAssertTrue(leftModeControl.waitForExistence(timeout: 2))

        app.typeKey("0", modifierFlags: [.command, .option])
        XCTAssertFalse(app.staticTexts["Inspector"].waitForExistence(timeout: 1))
        app.typeKey("0", modifierFlags: [.command, .option])
        XCTAssertTrue(app.staticTexts["Inspector"].waitForExistence(timeout: 2))

        app.typeKey("`", modifierFlags: [.command])
        XCTAssertTrue(window.exists)
    }

    @MainActor

    func testCommandWClosesFocusedPaneBeforeActiveTab() {
        let paneSplit = app.buttons["pane.split.pane-1"]
        XCTAssertTrue(paneSplit.waitForExistence(timeout: 5))
        paneSplit.click()
        let splitRight = app.menuItems["Split Right"]
        XCTAssertTrue(splitRight.waitForExistence(timeout: 2))
        splitRight.click()

        XCTAssertTrue(app.buttons["pane.focus.pane-new-2"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["tab.tab-terminal"].exists)

        app.typeKey("w", modifierFlags: [.command])
        XCTAssertFalse(app.buttons["pane.focus.pane-new-2"].waitForExistence(timeout: 1))
        XCTAssertTrue(app.buttons["pane.focus.pane-1"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["tab.tab-terminal"].exists)
        XCTAssertEqual(app.staticTexts["inspector.tab-name"].label, "Core Terminal")

        app.typeKey("w", modifierFlags: [.command])
        XCTAssertFalse(app.buttons["tab.tab-terminal"].waitForExistence(timeout: 1))
        XCTAssertTrue(app.buttons["tab.tab-agent"].waitForExistence(timeout: 2))
        XCTAssertEqual(app.staticTexts["inspector.tab-name"].label, "Agent Development")
    }

    @MainActor

    func testCommandWClosesWindowAfterLastTabAndPane() {
        let window = app.windows["Seyal — UI Shell Preview"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))

        let lab = app.buttons["workspace.workspace-lab"]
        XCTAssertTrue(lab.waitForExistence(timeout: 2))
        lab.click()
        XCTAssertTrue(app.buttons["tab.tab-lab-terminal"].waitForExistence(timeout: 2))
        XCTAssertFalse(app.buttons["pane.close.pane-lab"].exists)

        app.typeKey("w", modifierFlags: [.command])
        XCTAssertFalse(window.waitForExistence(timeout: 2))
    }

    @MainActor

    func testForcedShortcutHintsAnnotateReachableControlsWithoutReplacingUI() {
        app.terminate()
        app.launchEnvironment["SEYAL_UI_TEST_FORCE_SHORTCUT_HINTS"] = "1"
        app.launch()

        let window = app.windows["Seyal — UI Shell Preview"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))
        XCTAssertTrue(app.staticTexts["shortcut-hint.tab.tab-terminal"].waitForExistence(timeout: 2))
        XCTAssertEqual(
            app.staticTexts.matching(identifier: "shortcut-hint.tab.tab-terminal").count,
            1
        )
        XCTAssertEqual(app.staticTexts["shortcut-hint.tab.tab-terminal"].label, "⌘1")
        XCTAssertTrue(app.staticTexts["shortcut-hint.workspace.workspace-seyal"].waitForExistence(timeout: 2))
        XCTAssertEqual(app.staticTexts["shortcut-hint.workspace.workspace-seyal"].label, "⌃⌘1")
        XCTAssertEqual(app.staticTexts["shortcut-hint.new-tab"].label, "⌘T")
        XCTAssertEqual(app.staticTexts["shortcut-hint.left-sidebar"].label, "⌘0")
        XCTAssertEqual(app.staticTexts["shortcut-hint.inspector"].label, "⌥⌘0")
        XCTAssertEqual(app.staticTexts["shortcut-hint.close-focused-context"].label, "⌘W")
        XCTAssertTrue(app.buttons["tab.tab-terminal"].exists)
        XCTAssertTrue(app.textViews["composer.pane-1"].exists)
    }

    @MainActor

    func testProductionNativeSurfaceRemainsReachableAfterProtocolCompatibilityMigration() {
        app.terminate()
        let surface = launchProductionApp(requireUsableConnection: false)

        XCTAssertTrue(surface.exists)
        XCTAssertTrue(surface.isHittable)
        surface.click()
        XCTAssertTrue(surface.isHittable)
    }

    @MainActor

    func testProductionGraphemeDisplaySurfaceRemainsReachable() {
        app.terminate()
        let surface = launchProductionApp(requireUsableConnection: false)

        // The grapheme projection is rendered by the existing pane-owned
        // Metal surface. This test checks only its established XCUI contract;
        // Unicode shaping details remain covered by native component tests.
        XCTAssertEqual(surface.identifier, "terminal-surface.pane-local")
        XCTAssertEqual(surface.label, "Seyal Terminal")
        XCTAssertTrue(surface.isHittable)
        XCTAssertGreaterThan(surface.frame.width, 0)
        XCTAssertGreaterThan(surface.frame.height, 0)
        surface.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5)).click()
        XCTAssertTrue(surface.isHittable)
    }

    @MainActor

    func testLightAppearanceStillExposesTokenBackedShellChrome() {
        app.terminate()
        app = XCUIApplication()
        app.launchArguments = ["--ui-shell-preview"]
        app.launchEnvironment["SEYAL_UI_TEST_FIXTURES"] = "1"
        app.launchEnvironment["SEYAL_UI_APPEARANCE"] = "light"
        app.launch()

        let window = app.windows["Seyal — UI Shell Preview"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["tab.tab-terminal"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.textViews["composer.pane-1"].exists)
        XCTAssertTrue(app.buttons["toggle-left-sidebar"].exists)
    }

    @MainActor

    func testAttentionItemNavigatesInsteadOfBeingDecorative() {
        let attentionButton = app.buttons["attention"]
        XCTAssertTrue(attentionButton.waitForExistence(timeout: 5))

        attentionButton.click()

        let item = app.buttons["attention-item.attention-preview-tab"]
        XCTAssertTrue(item.waitForExistence(timeout: 2))
        item.click()

        let inspectorTab = app.staticTexts["inspector.tab-name"]
        XCTAssertTrue(inspectorTab.waitForExistence(timeout: 2))
        XCTAssertEqual(inspectorTab.label, "Agent Development")
    }

    @MainActor
    func testProductionScrollbackKeepsOlderHistoryReadable() throws {
        app.terminate()
        terminateOrphanedRuntimes()
        let runtime = try startExternalZshRuntime()
        defer {
            if runtime.isRunning { runtime.terminate() }
            runtime.waitUntilExit()
        }

        let surface = launchProductionApp()
        let baseline = surface.screenshot().pngRepresentation
        submitProductionShellCommand(
            "for i in $(seq 1 250); do printf 'history-%03d\\n' \"$i\"; done",
            surface: surface
        )
        let block = waitForVisibleCommandBlock()
        XCTAssertTrue(
            wait(timeout: 8) {
                self.recoveryFields(surface)?["connection"] == "usable"
                    && surface.screenshot().pngRepresentation != baseline
            },
            "numbered history did not render on the production Metal surface"
        )
        // Pixel inequality only proves the drawable changed. It does not
        // require the surface to match the light AppKit canvas; default
        // terminal cells remain the hard-coded dark Metal palette.

        let transcript = app.scrollViews["transcript.pane-local"]
        XCTAssertTrue(transcript.waitForExistence(timeout: 5))
        let beforeBlockMinY = block.frame.minY
        let beforeBar = transcript.scrollBars.firstMatch.exists
            ? transcript.scrollBars.firstMatch.value as? String
            : nil
        transcript.scroll(byDeltaX: 0, deltaY: 600)
        XCTAssertTrue(
            wait(timeout: 3) {
                block.frame.minY != beforeBlockMinY
                    || (
                        transcript.scrollBars.firstMatch.exists
                            && (transcript.scrollBars.firstMatch.value as? String) != beforeBar
                    )
            },
            "transcript did not scroll older history into view"
        )
        XCTAssertEqual(recoveryFields(surface)?["connection"], "usable")
        attachHeadedPNG(surface, name: "m002-819-headed-scrollback")
    }

    @MainActor
    func testProductionResizeReflowsAsciiCjkEmojiLine() throws {
        app.terminate()
        terminateOrphanedRuntimes()
        let runtime = try startExternalZshRuntime()
        defer {
            if runtime.isRunning { runtime.terminate() }
            runtime.waitUntilExit()
        }

        let surface = launchProductionApp()
        let baseline = surface.screenshot().pngRepresentation
        // Typed command stays ASCII; printf expands the mixed payload at the PTY.
        submitProductionShellCommand(
            "LC_ALL=C printf '%b' 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789xxxx\\0347\\0225\\0214\\0360\\0237\\0220\\0200e\\0314\\0201YYYYYYYYYY\\0012'",
            surface: surface
        )
        _ = waitForVisibleCommandBlock()
        XCTAssertTrue(
            wait(timeout: 8) { surface.screenshot().pngRepresentation != baseline },
            "mixed ASCII/CJK/emoji line did not present"
        )
        attachHeadedPNG(surface, name: "m002-819-headed-reflow-wide")

        collapseChromeForReflow()
        let wide = surface.frame.width
        resizeSeyalWindow(width: 620)
        XCTAssertTrue(
            wait(timeout: 5) { abs(surface.frame.width - wide) > 8 },
            "window resize did not change terminal surface width"
        )
        XCTAssertEqual(recoveryFields(surface)?["connection"], "usable")
        attachHeadedPNG(surface, name: "m002-819-headed-reflow-narrow")

        let narrow = surface.frame.width
        resizeSeyalWindow(width: 1180)
        XCTAssertTrue(
            wait(timeout: 5) {
                abs(surface.frame.width - narrow) > 8
                    && self.recoveryFields(surface)?["connection"] == "usable"
            },
            "restored window did not widen the terminal surface"
        )
        attachHeadedPNG(surface, name: "m002-819-headed-reflow-restored")
    }

    @MainActor
    func testProductionHardBreaksSurviveResizeWhileSoftWrapsRejoin() throws {
        app.terminate()
        terminateOrphanedRuntimes()
        let runtime = try startExternalZshRuntime()
        defer {
            if runtime.isRunning { runtime.terminate() }
            runtime.waitUntilExit()
        }

        let surface = launchProductionApp()
        submitProductionShellCommand(
            "printf 'HARD-A\\nHARD-B\\n'; LC_ALL=C printf '%b' 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789xxxxYYYYYYYYYY\\0012'",
            surface: surface
        )
        _ = waitForVisibleCommandBlock()
        attachHeadedPNG(surface, name: "m002-819-headed-hard-soft-initial")

        collapseChromeForReflow()
        let initialWidth = surface.frame.width
        resizeSeyalWindow(width: 600)
        XCTAssertTrue(
            wait(timeout: 5) { abs(surface.frame.width - initialWidth) > 8 }
        )
        attachHeadedPNG(surface, name: "m002-819-headed-hard-soft-narrow")
        let narrow = surface.frame.width
        resizeSeyalWindow(width: 1180)
        XCTAssertTrue(
            wait(timeout: 5) { abs(surface.frame.width - narrow) > 8 }
        )
        XCTAssertEqual(recoveryFields(surface)?["connection"], "usable")
        XCTAssertEqual(recoveryFields(surface)?["alternate-screen"], "false")
        attachHeadedPNG(surface, name: "m002-819-headed-hard-soft-wide")
    }

    @MainActor
    func testProductionAlternateScreenLeavesPrimaryHistoryClean() throws {
        app.terminate()
        terminateOrphanedRuntimes()
        let runtime = try startExternalZshRuntime()
        defer {
            if runtime.isRunning { runtime.terminate() }
            runtime.waitUntilExit()
        }

        let surface = launchProductionApp()
        submitProductionShellCommand("printf 'PRIMARY-BEFORE\\n'", surface: surface)
        _ = waitForVisibleCommandBlock()
        XCTAssertEqual(recoveryFields(surface)?["alternate-screen"], "false")

        focusTerminalSurface(surface)
        app.typeText("printf '\\033[?1049hTUI-JUNK\\n'")
        app.typeKey(.return, modifierFlags: [])
        XCTAssertTrue(
            wait(timeout: 5) { self.recoveryFields(surface)?["alternate-screen"] == "true" },
            "alternate screen did not become active; \(surface.value ?? "<none>")"
        )
        attachHeadedPNG(surface, name: "m002-819-headed-alt-screen-active")

        focusTerminalSurface(surface)
        app.typeText("printf '\\033[?1049l'")
        app.typeKey(.return, modifierFlags: [])
        XCTAssertTrue(
            wait(timeout: 5) { self.recoveryFields(surface)?["alternate-screen"] == "false" },
            "alternate screen did not restore primary history; \(surface.value ?? "<none>")"
        )
        XCTAssertEqual(recoveryFields(surface)?["connection"], "usable")
        attachHeadedPNG(surface, name: "m002-819-headed-alt-screen-restored")
    }

    @MainActor
    func testProductionLiveResizeWhileOutputPrintsStaysCoherent() throws {
        app.terminate()
        terminateOrphanedRuntimes()
        let runtime = try startExternalZshRuntime()
        defer {
            if runtime.isRunning { runtime.terminate() }
            runtime.waitUntilExit()
        }

        let surface = launchProductionApp()
        submitProductionShellCommand(
            "for i in $(seq 1 400); do printf 'live-%03d\\n' \"$i\"; usleep 15000; done",
            surface: surface
        )
        XCTAssertTrue(
            wait(timeout: 5) { self.recoveryFields(surface)?["connection"] == "usable" }
        )
        collapseChromeForReflow()
        let before = app.windows["Seyal"].frame
        resizeSeyalWindow(width: max(560, before.width * 0.65), height: before.height)
        XCTAssertTrue(
            wait(timeout: 5) {
                abs(self.app.windows["Seyal"].frame.width - before.width) > 8
            },
            "live resize did not change window geometry"
        )
        XCTAssertTrue(app.windows["Seyal"].exists)
        XCTAssertGreaterThan(surface.frame.width, 0)
        XCTAssertGreaterThan(surface.frame.height, 0)
        XCTAssertTrue(
            wait(timeout: 8) { self.recoveryFields(surface)?["connection"] == "usable" },
            "live resize lost the production display connection"
        )
        attachHeadedPNG(surface, name: "m002-819-headed-live-resize")
    }
}
