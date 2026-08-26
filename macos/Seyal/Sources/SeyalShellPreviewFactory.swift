import AppKit

#if DEBUG
@MainActor
enum SeyalShellPreviewFactory {
    static func make(frame: NSRect) -> SeyalShellView {
        let bodies = SeyalShellPreviewData.terminalLines.map {
            PreviewTerminalFixtureView(lines: $0) as NSView
        }
        return SeyalShellView(
            frame: frame,
            snapshot: SeyalShellPreviewData.snapshot,
            blocks: SeyalShellPreviewData.blocks,
            blockBodies: bodies
        )
    }
}
#endif
