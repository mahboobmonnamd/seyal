import AppKit

/// Leftover deprecated factory. Not the supported launch path (#890/#883).
/// Shared fixtures live in `seyal_client::fixtures` (#884).
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
