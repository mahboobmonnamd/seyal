import XCTest

final class SeyalShellUITests: XCTestCase {
    private var app: XCUIApplication!

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

    func testShellLaunchesWithFrozenCoreHierarchyWithoutFabricatedRuntimeOutput() {
        let window = app.windows["Seyal — UI Shell Preview"]
        XCTAssertTrue(window.waitForExistence(timeout: 5))
        XCTAssertGreaterThan(window.frame.width, 1200)
        XCTAssertGreaterThan(window.frame.height, 760)

        XCTAssertTrue(app.buttons["toggle-left-sidebar"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["toggle-inspector"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.segmentedControls["left-mode"].waitForExistence(timeout: 2))
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
        let mode = app.segmentedControls["left-mode"]
        XCTAssertTrue(mode.waitForExistence(timeout: 5))

        let tabsSegment = mode.buttons["Tabs"]
        XCTAssertTrue(tabsSegment.exists)
        tabsSegment.click()

        XCTAssertTrue(app.staticTexts["TABS"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["left-tab.tab-terminal"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["left-tab.tab-agent"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["left-new-tab"].waitForExistence(timeout: 2))
        XCTAssertFalse(app.staticTexts["AGENTS · SEYAL OSS"].exists)

        mode.buttons["Workspaces"].click()
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
        XCTAssertFalse(app.segmentedControls["left-mode"].waitForExistence(timeout: 1))

        let leftToggle = app.buttons["toggle-left-sidebar"]
        XCTAssertTrue(leftToggle.waitForExistence(timeout: 2))
        leftToggle.click()
        XCTAssertTrue(app.segmentedControls["left-mode"].waitForExistence(timeout: 2))

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
