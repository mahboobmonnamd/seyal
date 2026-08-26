import AppKit

/// UI-owned container for the permanent Metal terminal surface.
///
/// This view deliberately owns no terminal cells, VT state, PTY state, or copied
/// transcript. Pass 6 can bind the renderer-facing DisplayCache/RenderState to
/// the contained Metal surface without changing Block or shell ownership.
@MainActor
final class TerminalSurfaceHostView: NSView {
    let metalSurface: MetalSurfaceView

    override init(frame frameRect: NSRect) {
        metalSurface = MetalSurfaceView(frame: frameRect)
        super.init(frame: frameRect)
        translatesAutoresizingMaskIntoConstraints = false

        metalSurface.translatesAutoresizingMaskIntoConstraints = false
        addSubview(metalSurface)

        NSLayoutConstraint.activate([
            metalSurface.leadingAnchor.constraint(equalTo: leadingAnchor),
            metalSurface.trailingAnchor.constraint(equalTo: trailingAnchor),
            metalSurface.topAnchor.constraint(equalTo: topAnchor),
            metalSurface.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("TerminalSurfaceHostView is programmatic")
    }
}
