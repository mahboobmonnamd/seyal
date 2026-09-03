import AppKit

/// Creates the normal application presentation from the real bridge-backed
/// execution. Preview fixtures remain available only through the explicit
/// debug preview entry point.
@MainActor
enum SeyalShellProductionFactory {
    static func make(
        frame: NSRect,
        visual: SeyalResolvedVisualConfiguration
    ) -> SeyalShellView {
        let shell = SeyalShellView(
            frame: frame,
            state: SeyalShellState.makeProduction(),
            productionShell: true,
            visual: visual
        )
        shell.translatesAutoresizingMaskIntoConstraints = true
        shell.autoresizingMask = [.width, .height]
        return shell
    }
}
