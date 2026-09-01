import XCTest

final class SeyalShellUITests: XCTestCase {
    private var app: XCUIApplication!

    private var leftModeControl: XCUIElement {
        let segmentedControl = app.segmentedControls["left-mode"]
        return segmentedControl.exists
            ? segmentedControl
            : app.radioGroups["left-mode"]
    }

    private func leftModeSegment(_ label: String) -> XCUIElement {
        let button = leftModeControl.buttons[label]
        return button.exists ? button : leftModeControl.radioButtons[label]
    }

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--ui-shell-preview"]
        app.launchEnvironment["SEYAL_UI_TEST_FIXTURES"] = "1"
        app.launch()
    }

    override func tearDownWithError() throws {
        app.terminate()
        app = nil
    }

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

    func testProductionAppExecutesShellCommandThroughExternalRuntime() throws {
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

    func testProductionShellUsesOnePaneOwnedComposerAndMetalSurface() {
        // The production launch intentionally has no preview flag or fixture
        // environment. This exercises the real AppKit shell factory and its
        // pane-owned surface/composer identity, while remaining independent of
        // an optional Runtime process on the test host.
        app.terminate()
        app = XCUIApplication()
        app.launchArguments = []
        app.launchEnvironment = [:]
        app.launch()

        let window = app.windows["Seyal"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))

        let composer = app.textViews["composer.pane-local"]
        XCTAssertTrue(composer.waitForExistence(timeout: 3))
        let surfaces = app
            .descendants(matching: .any)
            .matching(identifier: "terminal-surface.pane-local")
        XCTAssertEqual(
            surfaces.count,
            1,
            "the production terminal surface remains discoverable at the recovery boundary"
        )

        composer.click()
        composer.typeText("printf 'pass7.1'")
        XCTAssertEqual(composer.value as? String, "printf 'pass7.1'")
    }

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

    func testTopTabActuallySwitchesActiveTabAndInspector() {
        let target = app.buttons["tab.tab-agent"]
        XCTAssertTrue(target.waitForExistence(timeout: 5))

        target.click()

        let inspectorTab = app.staticTexts["inspector.tab-name"]
        XCTAssertTrue(inspectorTab.waitForExistence(timeout: 2))
        XCTAssertEqual(inspectorTab.label, "Agent Development")
        XCTAssertTrue(app.textViews["composer.pane-agent"].waitForExistence(timeout: 2))
    }

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
}
