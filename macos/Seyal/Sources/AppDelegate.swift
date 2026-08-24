import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let contentRect = NSRect(x: 0, y: 0, width: 1440, height: 920)
        let shouldUseDesignPreview = CommandLine.arguments.contains("--design-preview")
            || ProcessInfo.processInfo.environment["SEYAL_DESIGN_PREVIEW"] == "1"

        let window = NSWindow(
            contentRect: contentRect,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )

        if shouldUseDesignPreview {
            window.title = "Seyal — Design Preview"
            window.contentView = DesignPreviewRootView(frame: contentRect)
        } else {
            window.title = "Seyal"
            let surface = MetalSurfaceView(frame: contentRect)
            window.contentView = surface
        }

        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

@MainActor
final class DesignPreviewRootView: NSView {
    enum Theme {
        case dark
        case light
    }

    enum State: String, CaseIterable {
        case singlePane = "Single"
        case longBlock = "Long block"
        case multiPane = "Multi pane"
        case tui = "TUI"
        case attention = "Attention"
    }

    private let selectorStack = NSStackView()
    private let workbench = DesignPreviewWorkbenchView()
    private var theme: Theme = .dark
    private var state: State = .singlePane

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        buildUI()
        applyTheme()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func buildUI() {
        selectorStack.orientation = .horizontal
        selectorStack.spacing = 8
        selectorStack.alignment = .centerY
        selectorStack.setFrameOrigin(NSPoint(x: 20, y: frame.height - 52))
        selectorStack.setFrameSize(NSSize(width: frame.width - 40, height: 28))

        let buttons = State.allCases.map { stateCase in
            let button = NSButton(title: stateCase.rawValue, target: self, action: #selector(handleStateSelection(_:)))
            button.tag = State.allCases.firstIndex(of: stateCase) ?? 0
            button.bezelStyle = .rounded
            button.font = NSFont.systemFont(ofSize: 12, weight: .medium)
            button.setButtonType(.momentaryPushIn)
            button.isBordered = true
            button.translatesAutoresizingMaskIntoConstraints = false
            button.widthAnchor.constraint(equalToConstant: 96).isActive = true
            button.heightAnchor.constraint(equalToConstant: 28).isActive = true
            return button
        }

        buttons.forEach { selectorStack.addArrangedSubview($0) }
        addSubview(selectorStack)

        workbench.frame = NSRect(x: 0, y: 0, width: frame.width, height: frame.height - 60)
        addSubview(workbench)
        workbench.state = state
        workbench.theme = theme
    }

    @objc
    private func handleStateSelection(_ sender: NSButton) {
        let index = sender.tag
        guard index >= 0, index < State.allCases.count else { return }
        state = State.allCases[index]
        workbench.state = state
        workbench.theme = theme
        updateButtonSelection()
    }

    private func updateButtonSelection() {
        for subview in selectorStack.arrangedSubviews.compactMap({ $0 as? NSButton }) {
            let isSelected = State.allCases[subview.tag] == state
            subview.isBordered = true
            subview.contentTintColor = isSelected ? .controlAccentColor : nil
            subview.alphaValue = isSelected ? 1.0 : 0.72
        }
    }

    private func applyTheme() {
        let darkBackground = NSColor(deviceRed: 0.06, green: 0.08, blue: 0.12, alpha: 1.0)
        let lightBackground = NSColor(deviceRed: 0.96, green: 0.97, blue: 0.99, alpha: 1.0)
        layer?.backgroundColor = (theme == .dark ? darkBackground : lightBackground).cgColor
        selectorStack.layer?.backgroundColor = NSColor.clear.cgColor
        workbench.theme = theme
        workbench.state = state
        updateButtonSelection()
    }

    override func layout() {
        super.layout()
        selectorStack.setFrameOrigin(NSPoint(x: 20, y: frame.height - 52))
        selectorStack.setFrameSize(NSSize(width: frame.width - 40, height: 28))
        workbench.frame = NSRect(x: 0, y: 0, width: frame.width, height: frame.height - 60)
    }
}

@MainActor
final class DesignPreviewWorkbenchView: NSView {
    var theme: DesignPreviewRootView.Theme = .dark
    var state: DesignPreviewRootView.State = .singlePane

    private let workspacePane = WorkspaceSidebarView()
    private let paneView = PreviewTerminalPaneView()

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        buildUI()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func buildUI() {
        workspacePane.frame = NSRect(x: 0, y: 0, width: 220, height: frame.height)
        paneView.frame = NSRect(x: 220, y: 0, width: max(0, frame.width - 220), height: frame.height)
        addSubview(workspacePane)
        addSubview(paneView)
    }

    override func layout() {
        super.layout()
        workspacePane.frame = NSRect(x: 0, y: 0, width: 220, height: frame.height)
        paneView.frame = NSRect(x: 220, y: 0, width: max(0, frame.width - 220), height: frame.height)
        workspacePane.theme = theme
        paneView.theme = theme
        paneView.state = state
    }
}

final class WorkspaceSidebarView: NSView {
    var theme: DesignPreviewRootView.Theme = .dark

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func draw(_ dirtyRect: NSRect) {
        let fill = theme == .dark ? NSColor(deviceRed: 0.10, green: 0.12, blue: 0.17, alpha: 1.0)
            : NSColor(deviceRed: 0.98, green: 0.98, blue: 0.99, alpha: 1.0)
        fill.setFill()
        dirtyRect.fill()

        let outline = theme == .dark ? NSColor(deviceRed: 0.23, green: 0.26, blue: 0.31, alpha: 1.0)
            : NSColor(deviceRed: 0.84, green: 0.86, blue: 0.90, alpha: 1.0)
        outline.setStroke()
        NSBezierPath(rect: NSRect(x: bounds.width - 1, y: 0, width: 1, height: bounds.height)).stroke()

        let titleColor = theme == .dark ? NSColor.white : NSColor.black
        titleColor.withAlphaComponent(0.9).setFill()
        let title = NSAttributedString(
            string: "workspace",
            attributes: [.font: NSFont.systemFont(ofSize: 12, weight: .semibold), .foregroundColor: titleColor.withAlphaComponent(0.75)]
        )
        title.draw(at: NSPoint(x: 18, y: bounds.height - 28))

        let rows: [(String, Bool)] = [
            ("seyal-demo", true),
            ("runtime", false),
            ("agent-routes", false),
            ("plans", false),
            ("support", false),
        ]

        var y = bounds.height - 58
        for (index, row) in rows.enumerated() {
            let item = NSRect(x: 18, y: y, width: bounds.width - 36, height: 28)
            let background = row.1 ? (theme == .dark ? NSColor(deviceRed: 0.19, green: 0.31, blue: 0.46, alpha: 1.0) : NSColor(deviceRed: 0.90, green: 0.94, blue: 0.99, alpha: 1.0)) : NSColor.clear
            background.setFill()
            NSBezierPath(roundedRect: item.insetBy(dx: 0, dy: 4), xRadius: 8, yRadius: 8).fill()
            let textColor = theme == .dark ? NSColor.white : NSColor.black
            let text = NSAttributedString(string: row.0, attributes: [.font: NSFont.systemFont(ofSize: 13, weight: index == 0 ? .semibold : .regular), .foregroundColor: textColor.withAlphaComponent(index == 0 ? 1.0 : 0.72)])
            text.draw(at: NSPoint(x: 28, y: item.minY + 7))
            y -= 34
        }
    }
}

final class PreviewTerminalPaneView: NSView {
    var theme: DesignPreviewRootView.Theme = .dark
    var state: DesignPreviewRootView.State = .singlePane

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func draw(_ dirtyRect: NSRect) {
        themeBackground().setFill(); dirtyRect.fill()

        let accent = theme == .dark ? NSColor(deviceRed: 0.27, green: 0.52, blue: 0.88, alpha: 1.0)
            : NSColor(deviceRed: 0.20, green: 0.44, blue: 0.84, alpha: 1.0)
        accent.withAlphaComponent(0.22).setFill()
        NSBezierPath(roundedRect: NSRect(x: 18, y: 16, width: bounds.width - 36, height: 34), xRadius: 12, yRadius: 12).fill()

        drawHeader()
        drawBlocks()
        drawComposer()
        if state == .tui { drawTUIOverlay() }
        if state == .attention { drawAttentionStack() }
    }

    private func themeBackground() -> NSColor {
        theme == .dark ? NSColor(deviceRed: 0.13, green: 0.17, blue: 0.22, alpha: 1.0) : NSColor(deviceRed: 0.97, green: 0.98, blue: 1.0, alpha: 1.0)
    }

    private func drawHeader() {
        let title = theme == .dark ? NSColor.white : NSColor.black
        let titleText = NSAttributedString(string: "seyal • runtime / active execution", attributes: [.font: NSFont.systemFont(ofSize: 12, weight: .semibold), .foregroundColor: title.withAlphaComponent(0.9)])
        titleText.draw(at: NSPoint(x: 32, y: bounds.height - 42))

        let pills = [
            ("blocks", theme == .dark ? NSColor(deviceRed: 0.18, green: 0.49, blue: 0.75, alpha: 1.0) : NSColor(deviceRed: 0.88, green: 0.94, blue: 0.99, alpha: 1.0)),
            ("ready", theme == .dark ? NSColor(deviceRed: 0.20, green: 0.58, blue: 0.45, alpha: 1.0) : NSColor(deviceRed: 0.91, green: 0.98, blue: 0.94, alpha: 1.0)),
        ]

        var x = bounds.width - 190
        for (label, color) in pills {
            let pill = NSRect(x: x, y: bounds.height - 42, width: 72, height: 22)
            color.setFill()
            NSBezierPath(roundedRect: pill, xRadius: 11, yRadius: 11).fill()
            let labelColor = theme == .dark ? NSColor.white : NSColor.black
            let attr = NSAttributedString(string: label, attributes: [.font: NSFont.systemFont(ofSize: 11, weight: .medium), .foregroundColor: labelColor])
            attr.draw(at: NSPoint(x: pill.minX + 16, y: pill.minY + 4))
            x += 84
        }
    }

    private func drawBlocks() {
        let baseY = bounds.height - 110
        let blockColors = [
            (NSColor(deviceRed: 0.18, green: 0.24, blue: 0.32, alpha: 1.0), 100, "(cmd) git status"),
            (NSColor(deviceRed: 0.17, green: 0.31, blue: 0.42, alpha: 1.0), 78, "(out) 14 files changed"),
            (NSColor(deviceRed: 0.19, green: 0.22, blue: 0.28, alpha: 1.0), 118, "(log) cargo check"),
            (NSColor(deviceRed: 0.30, green: 0.36, blue: 0.48, alpha: 1.0), 64, "(run) seyal app started")
        ]

        if state == .longBlock {
            let long = NSRect(x: 26, y: 118, width: bounds.width - 52, height: bounds.height - 208)
            let fill = theme == .dark ? NSColor(deviceRed: 0.18, green: 0.22, blue: 0.28, alpha: 1.0) : NSColor(deviceRed: 0.95, green: 0.96, blue: 0.99, alpha: 1.0)
            fill.setFill(); NSBezierPath(roundedRect: long, xRadius: 14, yRadius: 14).fill()
            let line = theme == .dark ? NSColor(deviceRed: 0.37, green: 0.45, blue: 0.58, alpha: 1.0) : NSColor(deviceRed: 0.84, green: 0.87, blue: 0.92, alpha: 1.0)
            line.setStroke(); NSBezierPath(rect: NSRect(x: long.minX + 10, y: long.minY + 8, width: 2, height: long.height - 16)).stroke()

            let textColor = theme == .dark ? NSColor.white : NSColor.black
            let message = "cargo check --workspace\nwarning: unused import\nnotes: 18 passed, 2 failed\nartifact: target/debug/seyal\n...\n...\n...\n...\n...\n...\n...\n...\n...\n...\n"
            let attributed = NSAttributedString(string: message, attributes: [.font: NSFont.userFixedPitchFont(ofSize: 12) ?? NSFont.systemFont(ofSize: 12), .foregroundColor: textColor.withAlphaComponent(0.82)])
            attributed.draw(in: NSRect(x: long.minX + 20, y: long.minY + 14, width: long.width - 30, height: long.height - 20))
            return
        }

        var y = baseY
        for (index, item) in blockColors.enumerated() {
            let fill = item.0
            let height = item.1
            let rect = NSRect(x: 26, y: y - CGFloat(index) * 8, width: bounds.width - 52, height: CGFloat(height))
            fill.setFill(); NSBezierPath(roundedRect: rect, xRadius: 12, yRadius: 12).fill()
            let status = index % 2 == 0 ? "run" : "done"
            let labelColor = theme == .dark ? NSColor.white : NSColor.black
            let attr = NSAttributedString(string: "  \(status)  \(item.2)", attributes: [.font: NSFont.systemFont(ofSize: 12, weight: .medium), .foregroundColor: labelColor.withAlphaComponent(0.9)])
            attr.draw(at: NSPoint(x: rect.minX + 14, y: rect.minY + 10))
            y -= CGFloat(height) + 8
        }

        if state == .multiPane {
            let second = NSRect(x: bounds.width * 0.55, y: 120, width: bounds.width * 0.32, height: bounds.height * 0.38)
            let fill = theme == .dark ? NSColor(deviceRed: 0.20, green: 0.26, blue: 0.34, alpha: 1.0) : NSColor(deviceRed: 0.93, green: 0.95, blue: 0.98, alpha: 1.0)
            fill.setFill(); NSBezierPath(roundedRect: second, xRadius: 12, yRadius: 12).fill()
            let label = NSAttributedString(string: "pane B • agent log", attributes: [.font: NSFont.systemFont(ofSize: 12, weight: .semibold), .foregroundColor: (theme == .dark ? NSColor.white : NSColor.black).withAlphaComponent(0.8)])
            label.draw(at: NSPoint(x: second.minX + 16, y: second.maxY - 28))
        }
    }

    private func drawComposer() {
        let composerRect = NSRect(x: 20, y: 20, width: bounds.width - 40, height: 100)
        let fill = theme == .dark ? NSColor(deviceRed: 0.09, green: 0.12, blue: 0.17, alpha: 1.0) : NSColor(deviceRed: 0.99, green: 0.99, blue: 1.0, alpha: 1.0)
        fill.setFill(); NSBezierPath(roundedRect: composerRect, xRadius: 14, yRadius: 14).fill()

        let border = theme == .dark ? NSColor(deviceRed: 0.24, green: 0.30, blue: 0.38, alpha: 1.0) : NSColor(deviceRed: 0.81, green: 0.85, blue: 0.90, alpha: 1.0)
        border.setStroke(); NSBezierPath(roundedRect: composerRect.insetBy(dx: 0.5, dy: 0.5), xRadius: 14, yRadius: 14).stroke()

        let prompt = NSAttributedString(string: "> cargo check --workspace", attributes: [.font: NSFont.userFixedPitchFont(ofSize: 13) ?? NSFont.systemFont(ofSize: 13), .foregroundColor: (theme == .dark ? NSColor.white : NSColor.black).withAlphaComponent(0.9)])
        prompt.draw(at: NSPoint(x: composerRect.minX + 12, y: composerRect.minY + 56))

        let run = NSRect(x: composerRect.maxX - 100, y: composerRect.minY + 12, width: 80, height: 28)
        let runColor = theme == .dark ? NSColor(deviceRed: 0.24, green: 0.51, blue: 0.90, alpha: 1.0) : NSColor(deviceRed: 0.18, green: 0.39, blue: 0.79, alpha: 1.0)
        runColor.setFill(); NSBezierPath(roundedRect: run, xRadius: 10, yRadius: 10).fill()
        let label = NSAttributedString(string: "Run", attributes: [.font: NSFont.systemFont(ofSize: 12, weight: .semibold), .foregroundColor: NSColor.white])
        label.draw(at: NSPoint(x: run.minX + 28, y: run.minY + 7))
    }

    private func drawTUIOverlay() {
        let overlay = NSRect(x: 40, y: 90, width: bounds.width - 80, height: bounds.height - 220)
        let fill = theme == .dark ? NSColor(deviceRed: 0.08, green: 0.11, blue: 0.15, alpha: 1.0) : NSColor(deviceRed: 0.92, green: 0.94, blue: 0.98, alpha: 1.0)
        fill.setFill(); NSBezierPath(roundedRect: overlay, xRadius: 14, yRadius: 14).fill()

        let text = theme == .dark ? NSColor.white : NSColor.black
        let simulated = "vim\n~/.config/seyal\n\n• workspace ready\n• local runtime live\n• alt-screen active"
        let attr = NSAttributedString(string: simulated, attributes: [.font: NSFont.userFixedPitchFont(ofSize: 12) ?? NSFont.systemFont(ofSize: 12), .foregroundColor: text.withAlphaComponent(0.84)])
        attr.draw(in: NSRect(x: overlay.minX + 18, y: overlay.minY + 12, width: overlay.width - 30, height: overlay.height - 24))
    }

    private func drawAttentionStack() {
        let stack = NSRect(x: bounds.width - 250, y: 94, width: 220, height: 180)
        let fill = theme == .dark ? NSColor(deviceRed: 0.16, green: 0.18, blue: 0.24, alpha: 0.96) : NSColor(deviceRed: 0.98, green: 0.98, blue: 1.0, alpha: 0.96)
        fill.setFill(); NSBezierPath(roundedRect: stack, xRadius: 14, yRadius: 14).fill()
        let border = theme == .dark ? NSColor(deviceRed: 0.46, green: 0.53, blue: 0.78, alpha: 1.0) : NSColor(deviceRed: 0.77, green: 0.82, blue: 0.91, alpha: 1.0)
        border.setStroke(); NSBezierPath(roundedRect: stack.insetBy(dx: 0.5, dy: 0.5), xRadius: 14, yRadius: 14).stroke()

        let textColor = theme == .dark ? NSColor.white : NSColor.black
        let items = [
            ("Approve tool call", NSColor(deviceRed: 0.25, green: 0.58, blue: 0.98, alpha: 1.0)),
            ("Answer question", NSColor(deviceRed: 0.33, green: 0.67, blue: 0.60, alpha: 1.0)),
            ("Review change", NSColor(deviceRed: 0.88, green: 0.58, blue: 0.29, alpha: 1.0))
        ]

        var y = stack.maxY - 36
        for (label, color) in items {
            color.setFill(); NSBezierPath(roundedRect: NSRect(x: stack.minX + 16, y: y - 52, width: stack.width - 32, height: 36), xRadius: 10, yRadius: 10).fill()
            let attr = NSAttributedString(string: label, attributes: [.font: NSFont.systemFont(ofSize: 12, weight: .medium), .foregroundColor: textColor])
            attr.draw(at: NSPoint(x: stack.minX + 28, y: y - 42))
            y -= 48
        }
    }
}
