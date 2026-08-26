import AppKit

#if DEBUG
@MainActor
enum SeyalShellPreviewFactory {
    static func make(
        frame: NSRect,
        state: SeyalShellPreviewState? = nil
    ) -> SeyalShellView {
        let resolvedState = state ?? SeyalShellPreviewState.makeDefault(
            includeTestAttention: ProcessInfo.processInfo.environment["SEYAL_UI_TEST_FIXTURES"] == "1"
        )
        let shell = SeyalShellView(frame: frame, state: resolvedState)
        shell.translatesAutoresizingMaskIntoConstraints = true
        shell.autoresizingMask = [.width, .height]
        return shell
    }
}
#endif
