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

        XCTAssertTrue(app.staticTexts["WORKSPACES"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["AGENTS"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["TABS"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["tab.tab-terminal"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["tab.tab-agent"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["tab.tab-logs"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["new-tab"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["split-right"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["split-down"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["Inspector"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["No TerminalExecution attached"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.staticTexts["No agent sessions"].waitForExistence(timeout: 2))
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
        let inspectorTab = app.staticTexts["inspector.tab-name"]
        XCTAssertTrue(inspectorTab.waitForExistence(timeout: 2))
        XCTAssertEqual(inspectorTab.label, "Terminal 5")
    }

    func testSplitRightCreatesSecondPaneAndUpdatesInspector() {
        let split = app.buttons["split-right"]
        XCTAssertTrue(split.waitForExistence(timeout: 5))

        split.click()

        XCTAssertTrue(app.buttons["pane.focus.pane-1"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.buttons["pane.focus.pane-new-2"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.textViews["composer.pane-1"].waitForExistence(timeout: 2))
        XCTAssertTrue(app.textViews["composer.pane-new-2"].waitForExistence(timeout: 2))

        let panes = app.staticTexts["inspector.tab-panes"]
        let layout = app.staticTexts["inspector.tab-layout"]
        XCTAssertTrue(panes.waitForExistence(timeout: 2))
        XCTAssertTrue(layout.waitForExistence(timeout: 2))
        XCTAssertEqual(panes.label, "2")
        XCTAssertEqual(layout.label, "Split right")
    }

    func testWorkspaceSelectionChangesWorkspaceScopedTabs() {
        let lab = app.buttons["workspace.workspace-lab"]
        XCTAssertTrue(lab.waitForExistence(timeout: 5))

        lab.click()

        XCTAssertTrue(app.buttons["tab.tab-lab-terminal"].waitForExistence(timeout: 2))
        XCTAssertFalse(app.buttons["tab.tab-agent"].exists)
        let workspace = app.staticTexts["inspector.workspace-name"]
        XCTAssertTrue(workspace.waitForExistence(timeout: 2))
        XCTAssertEqual(workspace.label, "Personal Lab")
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
