import AppKit

@MainActor
final class CommandBlockBodyView: NSView {
    private let outputView = NSTextView(frame: .zero)
    private weak var surface: InteractiveMetalSurfaceView?

    init(surface: InteractiveMetalSurfaceView? = nil, output: String = "") {
        self.surface = surface
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        outputView.translatesAutoresizingMaskIntoConstraints = false
        outputView.isEditable = false
        outputView.isSelectable = true
        outputView.isRichText = false
        outputView.drawsBackground = false
        outputView.font = SeyalDesignTokens.Typography.command
        outputView.textColor = SeyalDesignTokens.Palette.textSecondary
        outputView.string = output
        outputView.textContainerInset = NSSize(width: 0, height: 2)
        outputView.textContainer?.lineFragmentPadding = 0
        outputView.setAccessibilityLabel("Command output")
        addSubview(outputView)
        NSLayoutConstraint.activate([
            outputView.leadingAnchor.constraint(equalTo: leadingAnchor),
            outputView.trailingAnchor.constraint(equalTo: trailingAnchor),
            outputView.topAnchor.constraint(equalTo: topAnchor),
            outputView.bottomAnchor.constraint(equalTo: bottomAnchor),
            heightAnchor.constraint(greaterThanOrEqualToConstant: 28),
        ])
        if let surface {
            surface.isHidden = true
            addSubview(surface)
            NSLayoutConstraint.activate([
                surface.leadingAnchor.constraint(equalTo: leadingAnchor),
                surface.trailingAnchor.constraint(equalTo: trailingAnchor),
                surface.topAnchor.constraint(equalTo: topAnchor),
                surface.bottomAnchor.constraint(equalTo: bottomAnchor),
            ])
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("CommandBlockBodyView is programmatic") }

    func setOutput(_ output: String) {
        guard outputView.string != output else { return }
        outputView.string = output
        outputView.scrollToEndOfDocument(nil)
    }

    func setTUI(_ active: Bool) {
        outputView.isHidden = active
        surface?.isHidden = !active
    }

    static func text(from frame: NativePreparedFrame) -> String {
        var rows: [String] = []
        for row in 0..<frame.rows {
            var line = ""
            for column in 0..<frame.columns {
                let scalar = frame.cells[row * frame.columns + column].scalar
                line.append(UnicodeScalar(scalar).map(Character.init) ?? " ")
            }
            rows.append(line.trimmingCharacters(in: .whitespaces))
        }
        while rows.last?.isEmpty == true { rows.removeLast() }
        return rows.joined(separator: "\n")
    }
}
