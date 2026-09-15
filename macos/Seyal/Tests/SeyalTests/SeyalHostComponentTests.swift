import AppKit
import XCTest

@testable import Seyal

final class SeyalHostComponentTests: XCTestCase {
    func testApplicationRootABIMatchesPublishedHeader() {
        XCTAssertEqual(MemoryLayout<SeyalAppAction>.size, Int(MemoryLayout<SeyalAppAction>.stride))
        XCTAssertGreaterThanOrEqual(MemoryLayout<SeyalAppAction>.size, 80)
        XCTAssertGreaterThanOrEqual(MemoryLayout<SeyalAppSnapshot>.size, 64)
        let handle = seyal_app_create()
        XCTAssertNotEqual(handle, 0)
        let snapshot = seyal_app_snapshot(handle)
        XCTAssertEqual(snapshot.version, UInt16(SEYAL_APP_ABI_VERSION))
        XCTAssertEqual(snapshot.eligibility, UInt16(SEYAL_APP_ELIGIBILITY_UNBOUND.rawValue))
        XCTAssertEqual(MemoryLayout<SeyalAppBlockSpan>.size, 16)
        let emptySpan = seyal_app_block_span(handle, 0)
        XCTAssertEqual(emptySpan.start_line, 0)
        XCTAssertEqual(emptySpan.end_line, 0)
        XCTAssertEqual(seyal_app_destroy(handle), 0)
        let theme = seyal_app_theme(0)
        XCTAssertNotEqual(theme.canvas, theme.text)
        XCTAssertEqual(MemoryLayout<SeyalAppComposer>.size, 40)
        XCTAssertEqual(MemoryLayout<SeyalAppChrome>.size, 24)
        XCTAssertEqual(MemoryLayout<SeyalAppShell>.size, 64)
        XCTAssertEqual(MemoryLayout<SeyalAppRow>.size, 56)
        let live = seyal_app_create()
        let chrome = seyal_app_chrome(live)
        XCTAssertEqual(chrome.reserved & UInt32(SEYAL_APP_CHROME_LEFT_VISIBLE), 0)
        XCTAssertEqual(chrome.reserved & UInt32(SEYAL_APP_CHROME_INSPECTOR_VISIBLE), 0)
        XCTAssertEqual(chrome.reserved & UInt32(SEYAL_APP_CHROME_TAB_STRIP_VISIBLE), 0)
        let shell = seyal_app_shell(live)
        XCTAssertEqual(shell.workspace_count, 1)
        let workspace = seyal_app_shell_row(live, UInt16(SEYAL_APP_ROW_WORKSPACE), 0)
        XCTAssertGreaterThan(workspace.title_len, 0)
        XCTAssertEqual(seyal_app_destroy(live), 0)
        let dark = seyal_app_theme(0)
        let light = seyal_app_theme(1)
        XCTAssertNotEqual(dark.canvas, light.canvas)
        XCTAssertNotEqual(dark.canvas, dark.text)
        XCTAssertNotEqual(dark.accent, 0)
    }

    func testHostHasNoSeyalShellProductTypes() {
        let sourceRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("Sources", isDirectory: true)
        let enumerator = FileManager.default.enumerator(at: sourceRoot, includingPropertiesForKeys: nil)!
        var hits: [String] = []
        for case let file as URL in enumerator where file.pathExtension == "swift" {
            let text = (try? String(contentsOf: file, encoding: .utf8)) ?? ""
            let forbidden = [
                "SeyalShellState",
                "SeyalShellView",
                "SeyalShellPreviewFactory",
                "SeyalShellProductionFactory",
                "SeyalShellModel",
                "PanePresentationSession",
            ]
            if forbidden.contains(where: { text.contains($0) }) {
                hits.append(file.lastPathComponent)
            }
        }
        XCTAssertTrue(hits.isEmpty, "portable product types leaked into \(hits)")
    }

    func testNativeTestsConsumeRustApplicationRootFixturesOnly() {
        let handle = seyal_app_create()
        defer { XCTAssertEqual(seyal_app_destroy(handle), 0) }
        let shell = seyal_app_shell(handle)
        XCTAssertEqual(shell.workspace_count, 1)
        XCTAssertEqual(shell.tab_count, 1)
        XCTAssertEqual(shell.pane_count, 1)
        let chrome = seyal_app_chrome(handle)
        XCTAssertEqual(chrome.left_panel, 0)
        XCTAssertEqual(chrome.reserved, 0)
        let composer = seyal_app_composer(handle)
        XCTAssertEqual(composer.mode, UInt16(SEYAL_APP_COMPOSER_HIDDEN.rawValue))
        let placeholder = seyal_app_copy(handle, UInt16(SEYAL_APP_COPY_COMPOSER_PLACEHOLDER))
        let execute = seyal_app_copy(handle, UInt16(SEYAL_APP_COPY_COMPOSER_EXECUTE))
        let prompt = seyal_app_copy(handle, UInt16(SEYAL_APP_COPY_BLOCK_PROMPT))
        XCTAssertEqual(utf8(placeholder), "Type a command...")
        XCTAssertEqual(utf8(execute), "⏎")
        XCTAssertEqual(utf8(prompt), "$")
        let inspector = seyal_app_chrome_row(handle, UInt16(SEYAL_APP_ROW_INSPECTOR), 0)
        XCTAssertGreaterThan(inspector.title_len, 0)
    }

    func testRefreshAlternateScreenAfterBindDerivesTui() {
        let handle = seyal_app_create()
        defer { XCTAssertEqual(seyal_app_destroy(handle), 0) }
        var snap = seyal_app_snapshot(handle)
        var bind = SeyalAppAction()
        bind.version = UInt16(SEYAL_APP_ABI_VERSION)
        bind.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        bind.kind = UInt16(SEYAL_APP_ACTION_BIND.rawValue)
        bind.flags = UInt16(SEYAL_APP_FLAG_TARGET_CONTROLLER)
        bind.fence_pane_lo = snap.pane_lo
        bind.fence_pane_hi = snap.pane_hi
        bind.fence_epoch = snap.epoch
        bind.target_execution_lo = 1
        bind.target_attachment_lo = 2
        bind.target_pty_generation = 1
        XCTAssertEqual(seyal_app_apply(handle, &bind), 0)
        snap = seyal_app_snapshot(handle)
        XCTAssertEqual(snap.eligibility, UInt16(SEYAL_APP_ELIGIBILITY_FLOW.rawValue))
        var refresh = SeyalAppAction()
        refresh.version = bind.version
        refresh.size = bind.size
        refresh.kind = UInt16(SEYAL_APP_ACTION_REFRESH.rawValue)
        refresh.applySnapshotFence(snap)
        refresh.flags |= UInt16(SEYAL_APP_FLAG_ALTERNATE_SCREEN)
        XCTAssertEqual(seyal_app_apply(handle, &refresh), 0)
        let tui = seyal_app_snapshot(handle)
        XCTAssertEqual(tui.eligibility, UInt16(SEYAL_APP_ELIGIBILITY_TUI.rawValue))
        XCTAssertEqual(tui.flags & UInt16(SEYAL_APP_SNAP_COMPOSER), 0)
        XCTAssertEqual(
            seyal_app_composer(handle).mode,
            UInt16(SEYAL_APP_COMPOSER_HIDDEN.rawValue)
        )
    }

    @MainActor
    func testProductChromeReconcileIsReentrant() {
        let view = ProductChromeHostView(frame: NSRect(x: 0, y: 0, width: 800, height: 560))
        view.reconcileChrome()
        view.reconcileChrome()
    }

    func testBundledRuntimeLauncherUsesFixedHelperPath() {
        XCTAssertEqual(BundledRuntimeLauncher.helperRelativePath, "Contents/Helpers/seyal-runtime")
        XCTAssertEqual(BundledRuntimeLauncher.helperIdentifier, "dev.seyal.Seyal.runtime")
    }

    func testTranscriptFrameRejectsZeroBlockIdentity() {
        let invalid = NativeTranscriptFrame(
            revision: 1,
            regions: [NativeTranscriptRegion(id: 0, origin: .zero, clip: .zero)]
        )
        XCTAssertFalse(invalid.isValid)
        let valid = NativeTranscriptFrame(
            revision: 1,
            regions: [
                NativeTranscriptRegion(
                    id: 7,
                    origin: NSPoint(x: 0, y: 12),
                    clip: NSRect(x: 0, y: 12, width: 80, height: 24)
                )
            ]
        )
        XCTAssertTrue(valid.isValid)
        XCTAssertEqual(valid.regionIDs, [7])
    }

    // MARK: - Composer history (#933)

    /// Bind one Pane and drive an accepted composer submit through the FFI so
    /// Rust records history. Tests never fabricate rows.
    private func boundHandleWithHistory(_ commands: [String]) -> UInt64 {
        let handle = seyal_app_create()
        let snap = seyal_app_snapshot(handle)
        var bind = SeyalAppAction()
        bind.version = UInt16(SEYAL_APP_ABI_VERSION)
        bind.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        bind.kind = UInt16(SEYAL_APP_ACTION_BIND.rawValue)
        bind.flags = UInt16(SEYAL_APP_FLAG_TARGET_CONTROLLER)
        bind.fence_pane_lo = snap.pane_lo
        bind.fence_pane_hi = snap.pane_hi
        bind.fence_epoch = snap.epoch
        bind.target_execution_lo = 1
        bind.target_attachment_lo = 2
        bind.target_pty_generation = 1
        XCTAssertEqual(seyal_app_apply(handle, &bind), 0)
        for command in commands {
            let bound = seyal_app_snapshot(handle)
            let composer = seyal_app_composer(handle)
            var draft = SeyalAppAction()
            draft.version = bind.version
            draft.size = bind.size
            draft.kind = UInt16(SEYAL_APP_ACTION_SET_COMPOSER_DRAFT.rawValue)
            draft.applySnapshotFence(bound)
            draft.target_pty_generation = composer.epoch
            let utf8 = Array(command.utf8)
            utf8.withUnsafeBufferPointer { buffer in
                draft.payload = buffer.baseAddress
                draft.payload_len = UInt32(buffer.count)
                XCTAssertEqual(seyal_app_apply(handle, &draft), 0)
            }
            var submit = SeyalAppAction()
            submit.version = bind.version
            submit.size = bind.size
            submit.kind = UInt16(SEYAL_APP_ACTION_SUBMIT_COMPOSER.rawValue)
            submit.applySnapshotFence(bound)
            submit.target_pty_generation = composer.epoch
            XCTAssertEqual(seyal_app_apply(handle, &submit), 0)
            var result = SeyalAppAction()
            result.version = bind.version
            result.size = bind.size
            result.kind = UInt16(SEYAL_APP_ACTION_APPLY_COMPOSER_RESULT.rawValue)
            result.applySnapshotFence(bound)
            result.target_execution_lo = seyal_app_composer(handle).request_id
            result.reserved = 1
            XCTAssertEqual(seyal_app_apply(handle, &result), 0)
        }
        return handle
    }

    private func applyHistory(_ handle: UInt64, kind: UInt16, payload: String? = nil, reserved: UInt32 = 0) -> Int32 {
        let snap = seyal_app_snapshot(handle)
        var action = SeyalAppAction()
        action.version = UInt16(SEYAL_APP_ABI_VERSION)
        action.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        action.kind = kind
        action.applySnapshotFence(snap)
        action.reserved = reserved
        action.target_pty_generation = seyal_app_composer(handle).epoch
        let utf8 = Array((payload ?? "").utf8)
        return utf8.withUnsafeBufferPointer { buffer in
            action.payload = payload == nil ? nil : buffer.baseAddress
            action.payload_len = payload == nil ? 0 : UInt32(buffer.count)
            return seyal_app_apply(handle, &action)
        }
    }

    func testComposerHistoryABIMatchesPublishedHeader() {
        XCTAssertEqual(MemoryLayout<SeyalAppComposerHistory>.size, 32)
        XCTAssertEqual(MemoryLayout<SeyalAppComposerHistory>.stride, 32)
        XCTAssertEqual(MemoryLayout<SeyalAppComposerHistory>.offset(of: \.query_utf8), 16)
        let handle = seyal_app_create()
        defer { XCTAssertEqual(seyal_app_destroy(handle), 0) }
        let closed = seyal_app_composer_history(handle)
        XCTAssertEqual(closed.version, UInt16(SEYAL_APP_ABI_VERSION))
        XCTAssertEqual(Int(closed.size), MemoryLayout<SeyalAppComposerHistory>.size)
        XCTAssertEqual(closed.flags, 0)
        XCTAssertEqual(closed.entry_count, 0)
        XCTAssertEqual(seyal_app_history_row(handle, 0).title_len, 0)
        let label = seyal_app_copy(handle, UInt16(SEYAL_APP_COPY_COMPOSER_HISTORY))
        XCTAssertGreaterThan(label.title_len, 0)
        let placeholder = seyal_app_copy(handle, UInt16(SEYAL_APP_COPY_COMPOSER_HISTORY_PLACEHOLDER))
        XCTAssertGreaterThan(placeholder.title_len, 0)
        // Unbound composer is not Available: open fails closed in Rust.
        XCTAssertEqual(applyHistory(handle, kind: UInt16(SEYAL_APP_ACTION_OPEN_COMPOSER_HISTORY.rawValue)), -4)
        XCTAssertEqual(seyal_app_composer_history(handle).flags & UInt16(SEYAL_APP_HISTORY_OPEN), 0)
    }

    func testComposerHistoryRowsAreRustRecordedAcceptedSubmits() {
        let handle = boundHandleWithHistory(["cargo build", "git status"])
        defer { XCTAssertEqual(seyal_app_destroy(handle), 0) }
        let recorded = seyal_app_composer_history(handle)
        XCTAssertEqual(recorded.flags, UInt16(SEYAL_APP_HISTORY_HAS_ENTRIES))
        XCTAssertEqual(recorded.entry_count, 2)
        XCTAssertEqual(recorded.row_count, 0, "closed overlay projects no rows")
        XCTAssertEqual(applyHistory(handle, kind: UInt16(SEYAL_APP_ACTION_OPEN_COMPOSER_HISTORY.rawValue)), 0)
        let open = seyal_app_composer_history(handle)
        XCTAssertEqual(open.flags, UInt16(SEYAL_APP_HISTORY_OPEN | SEYAL_APP_HISTORY_HAS_ENTRIES))
        XCTAssertEqual(open.row_count, 2)
        XCTAssertEqual(utf8(seyal_app_history_row(handle, 0)), "git status")
        XCTAssertEqual(seyal_app_history_row(handle, 0).flags & UInt16(SEYAL_APP_ROW_SELECTED), UInt16(SEYAL_APP_ROW_SELECTED))
        XCTAssertEqual(applyHistory(handle, kind: UInt16(SEYAL_APP_ACTION_SET_COMPOSER_HISTORY_FILTER.rawValue), payload: "carg"), 0)
        XCTAssertEqual(seyal_app_composer_history(handle).row_count, 1)
        XCTAssertEqual(utf8(seyal_app_history_row(handle, 0)), "cargo build")
        XCTAssertEqual(applyHistory(handle, kind: UInt16(SEYAL_APP_ACTION_SELECT_COMPOSER_HISTORY.rawValue)), 0)
        let composer = seyal_app_composer(handle)
        XCTAssertEqual(composer.request_id, 0, "select inserts into the draft; it never submits")
        let draft = String(decoding: UnsafeBufferPointer(start: composer.draft_utf8, count: Int(composer.draft_utf8_len)), as: UTF8.self)
        XCTAssertEqual(draft, "cargo build")
        XCTAssertEqual(seyal_app_composer_history(handle).flags & UInt16(SEYAL_APP_HISTORY_OPEN), 0)
    }

    @MainActor
    func testComposerHistoryOverlayProjectsRustStateOnly() {
        let handle = boundHandleWithHistory(["echo one", "echo two"])
        defer { XCTAssertEqual(seyal_app_destroy(handle), 0) }
        let overlay = ComposerHistoryOverlayView(appHandle: handle)
        overlay.reconcile()
        XCTAssertTrue(overlay.isHidden, "overlay is hidden until Rust opens it")
        XCTAssertEqual(applyHistory(handle, kind: UInt16(SEYAL_APP_ACTION_OPEN_COMPOSER_HISTORY.rawValue)), 0)
        overlay.reconcile()
        XCTAssertFalse(overlay.isHidden)
        XCTAssertEqual(overlay.accessibilityValue() as? String, "2")
        XCTAssertEqual(applyHistory(handle, kind: UInt16(SEYAL_APP_ACTION_MOVE_COMPOSER_HISTORY_SELECTION.rawValue), reserved: 1), 0)
        var dismissed = 0
        overlay.onDismissed = { dismissed += 1 }
        overlay.reconcile()
        overlay.reconcile()
        XCTAssertEqual(dismissed, 0)
        XCTAssertEqual(applyHistory(handle, kind: UInt16(SEYAL_APP_ACTION_CLOSE_COMPOSER_HISTORY.rawValue)), 0)
        overlay.reconcile()
        XCTAssertTrue(overlay.isHidden)
        XCTAssertEqual(dismissed, 1, "closing notifies the host exactly once")
        overlay.reconcile()
        XCTAssertEqual(dismissed, 1)
    }

    @MainActor
    func testProductChromeHostsHiddenHistoryOverlayAndReconciles() {
        let view = ProductChromeHostView(frame: NSRect(x: 0, y: 0, width: 800, height: 560))
        view.reconcileChrome()
        XCTAssertTrue(view.historyOverlay.isHidden)
        XCTAssertNotNil(view.historyOverlay.superview)
        view.reconcileChrome()
        XCTAssertTrue(view.historyOverlay.isHidden)
    }

    // MARK: - Command palette (#932)

    func testCommandPaletteABIMatchesPublishedHeaderAndFailsClosedForBogusRow() {
        XCTAssertEqual(MemoryLayout<SeyalAppPalette>.size, 32)
        XCTAssertEqual(MemoryLayout<SeyalAppPalette>.stride, 32)
        XCTAssertEqual(MemoryLayout<SeyalAppPalette>.offset(of: \.query_utf8), 16)
        let handle = seyal_app_create()
        defer { XCTAssertEqual(seyal_app_destroy(handle), 0) }
        let closed = seyal_app_palette(handle)
        XCTAssertEqual(closed.version, UInt16(SEYAL_APP_ABI_VERSION))
        XCTAssertEqual(Int(closed.size), MemoryLayout<SeyalAppPalette>.size)
        XCTAssertEqual(closed.flags, 0)
        XCTAssertEqual(closed.row_count, 0)
        XCTAssertTrue(closed.query_utf8 == nil)
        XCTAssertEqual(seyal_app_palette_row(handle, 0).title_len, 0, "no row at any index while closed")
    }

    func testCommandPaletteOpenListsCommandsWithoutBindingAndOmitsDisallowedOnes() {
        let handle = seyal_app_create()
        defer { XCTAssertEqual(seyal_app_destroy(handle), 0) }
        let snap = seyal_app_snapshot(handle)
        var open = SeyalAppAction()
        open.version = UInt16(SEYAL_APP_ABI_VERSION)
        open.size = UInt16(MemoryLayout<SeyalAppAction>.size)
        open.kind = UInt16(SEYAL_APP_ACTION_OPEN_PALETTE.rawValue)
        open.applySnapshotFence(snap)
        // Unbound handle: require_fence only needs the Pane to exist, so the
        // palette opens before any Runtime attach.
        XCTAssertEqual(seyal_app_apply(handle, &open), 0)
        let palette = seyal_app_palette(handle)
        XCTAssertNotEqual(palette.flags & UInt16(SEYAL_APP_PALETTE_OPEN), 0)
        XCTAssertGreaterThan(palette.row_count, 0)
        var sawNewTab = false
        for index in 0..<Int(palette.row_count) {
            let row = seyal_app_palette_row(handle, UInt32(index))
            if utf8(row) == "New Tab" { sawNewTab = true }
        }
        XCTAssertFalse(
            sawNewTab,
            "M001 default shell policy disallows tab creation; the command is omitted, not disabled"
        )
    }

}

private func utf8(_ row: SeyalAppRow) -> String {
    guard row.title_len > 0, let title = row.title else { return "" }
    return String(decoding: UnsafeBufferPointer(start: title, count: Int(row.title_len)), as: UTF8.self)
}
