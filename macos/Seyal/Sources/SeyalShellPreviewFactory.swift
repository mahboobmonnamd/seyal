import AppKit

#if DEBUG
@MainActor
enum SeyalShellPreviewFactory {
    static func make(frame: NSRect) -> SeyalShellView {
        let bodies = SeyalShellPreviewData.terminalLines.map {
            PreviewTerminalFixtureView(lines: $0) as NSView
        }
        let shell = SeyalShellView(
            frame: frame,
            snapshot: SeyalShellPreviewData.snapshot,
            blocks: SeyalShellPreviewData.blocks,
            blockBodies: bodies
        )
        shell.translatesAutoresizingMaskIntoConstraints = true
        shell.autoresizingMask = [.width, .height]
        return shell
    }
}
#endif
