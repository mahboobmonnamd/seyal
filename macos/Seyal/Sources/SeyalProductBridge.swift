import Foundation

/// Thin FFI adapter over the Rust product session. Swift must not invent
/// Workspace/Tab/Pane/composer product state here.
@MainActor
final class SeyalProductBridge {
    struct Snapshot: Equatable {
        var workspaceName: String
        var tabTitle: String
        var paneTitle: String
        var draft: String
        var canSubmit: Bool
        var composerHidden: Bool
        var composerBusy: Bool
        var leftPanelTabs: Bool
        var presentationMode: UInt8
    }

    init() {
        _ = seyal_product_create()
    }

    func setDraft(_ text: String) {
        let bytes = Array(text.utf8)
        bytes.withUnsafeBufferPointer { buffer in
            _ = seyal_product_set_draft(buffer.baseAddress, UInt32(buffer.count))
        }
    }

    func submit() {
        _ = seyal_product_submit()
    }

    func setLeftPanelTabs(_ tabs: Bool) {
        _ = seyal_product_set_left_panel(tabs ? 1 : 0)
    }

    func snapshot() -> Snapshot {
        let raw = seyal_product_snapshot()
        return Snapshot(
            workspaceName: cString(raw.workspace_name),
            tabTitle: cString(raw.tab_title),
            paneTitle: cString(raw.pane_title),
            draft: cString(raw.draft),
            canSubmit: raw.can_submit != 0,
            composerHidden: raw.composer_mode == 2,
            composerBusy: raw.composer_mode == 1,
            leftPanelTabs: raw.left_panel_tabs != 0,
            presentationMode: raw.presentation_mode
        )
    }
}

private func cString<T>(_ value: T) -> String {
    withUnsafeBytes(of: value) { buffer in
        String(decoding: buffer.prefix { $0 != 0 }, as: UTF8.self)
    }
}
