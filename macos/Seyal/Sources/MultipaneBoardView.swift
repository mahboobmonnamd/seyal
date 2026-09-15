import AppKit

/// Thin projection of the Rust Tab PaneTree. Focused leaf hosts the live
/// terminal/composer surface; other leaves are focusable chrome regions.
@MainActor
final class MultipaneBoardView: NSView {
    weak var chromeHost: ProductChromeHostView?
    private let liveSurface = NSView()
    private var rootView: NSView?
    private var leafViews: [UInt64: PaneLeafView] = [:]
    private var focusedKey: UInt64?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        translatesAutoresizingMaskIntoConstraints = false
        setAccessibilityIdentifier("seyal-multipane-board")
        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        liveSurface.translatesAutoresizingMaskIntoConstraints = false
        liveSurface.setAccessibilityIdentifier("seyal-live-pane-surface")
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    var liveContentView: NSView { liveSurface }

    func installLiveSubviews(_ views: [NSView]) {
        liveSurface.subviews.forEach { $0.removeFromSuperview() }
        for view in views {
            view.translatesAutoresizingMaskIntoConstraints = false
            liveSurface.addSubview(view)
        }
        guard views.count == 3 else { return }
        let top = views[0]
        let middle = views[1]
        let bottom = views[2]
        NSLayoutConstraint.activate([
            top.leadingAnchor.constraint(equalTo: liveSurface.leadingAnchor, constant: 20),
            top.trailingAnchor.constraint(equalTo: liveSurface.trailingAnchor, constant: -20),
            top.topAnchor.constraint(equalTo: liveSurface.topAnchor, constant: 16),
            top.heightAnchor.constraint(greaterThanOrEqualToConstant: 240),
            middle.leadingAnchor.constraint(equalTo: liveSurface.leadingAnchor),
            middle.trailingAnchor.constraint(equalTo: liveSurface.trailingAnchor),
            middle.topAnchor.constraint(equalTo: top.topAnchor),
            middle.bottomAnchor.constraint(equalTo: top.bottomAnchor),
            bottom.leadingAnchor.constraint(equalTo: liveSurface.leadingAnchor, constant: 24),
            bottom.trailingAnchor.constraint(equalTo: liveSurface.trailingAnchor, constant: -24),
            bottom.topAnchor.constraint(equalTo: top.bottomAnchor, constant: 12),
            bottom.bottomAnchor.constraint(equalTo: liveSurface.bottomAnchor, constant: -16),
        ])
    }

    func rebuild(appHandle: UInt64) {
        let count = seyal_app_layout_count(appHandle)
        var nodes: [SeyalAppLayoutNode] = []
        nodes.reserveCapacity(Int(count))
        for index in 0..<count {
            nodes.append(seyal_app_layout_node(appHandle, index))
        }
        let shell = seyal_app_shell(appHandle)
        let focused = shell.focused_pane_lo
        rootView?.removeFromSuperview()
        leafViews.removeAll()
        guard !nodes.isEmpty else { return }
        let built = buildNode(0, nodes: nodes, focused: focused)
        rootView = built
        addSubview(built)
        NSLayoutConstraint.activate([
            built.leadingAnchor.constraint(equalTo: leadingAnchor),
            built.trailingAnchor.constraint(equalTo: trailingAnchor),
            built.topAnchor.constraint(equalTo: topAnchor),
            built.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
        attachLiveSurface(focused: focused)
    }

    private func buildNode(_ index: Int, nodes: [SeyalAppLayoutNode], focused: UInt64) -> NSView {
        let node = nodes[index]
        switch node.kind {
        case UInt16(SEYAL_APP_LAYOUT_SPLIT_RIGHT), UInt16(SEYAL_APP_LAYOUT_SPLIT_DOWN):
            let split = NSSplitView()
            split.isVertical = node.kind == UInt16(SEYAL_APP_LAYOUT_SPLIT_RIGHT)
            split.dividerStyle = .thin
            split.translatesAutoresizingMaskIntoConstraints = false
            split.setAccessibilityIdentifier("seyal-split-\(index)")
            let first = buildNode(Int(node.first_child), nodes: nodes, focused: focused)
            let second = buildNode(Int(node.second_child), nodes: nodes, focused: focused)
            split.addSubview(first)
            split.addSubview(second)
            return split
        default:
            let leaf = PaneLeafView(
                paneLo: node.pane_lo,
                paneHi: node.pane_hi,
                focused: node.pane_lo == focused
            )
            leaf.board = self
            leafViews[node.pane_lo] = leaf
            return leaf
        }
    }

    private func attachLiveSurface(focused: UInt64) {
        liveSurface.removeFromSuperview()
        focusedKey = focused
        guard let leaf = leafViews[focused] else { return }
        leaf.installLiveSurface(liveSurface)
    }

    func focusPane(lo: UInt64, hi: UInt64) {
        chromeHost?.focusPaneIdentity(lo: lo, hi: hi)
    }
}

@MainActor
final class PaneLeafView: NSView {
    weak var board: MultipaneBoardView?
    private let paneLo: UInt64
    private let paneHi: UInt64
    private let placeholder = NSButton(title: "Pane", target: nil, action: nil)
    private var isFocusedLeaf = false

    init(paneLo: UInt64, paneHi: UInt64, focused: Bool) {
        self.paneLo = paneLo
        self.paneHi = paneHi
        self.isFocusedLeaf = focused
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true
        setAccessibilityIdentifier("seyal-pane-leaf-\(paneLo)")
        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        placeholder.bezelStyle = .inline
        placeholder.isBordered = false
        placeholder.font = .systemFont(ofSize: 12, weight: .medium)
        placeholder.title = focused ? "Focused Pane" : "Focus Pane"
        placeholder.setAccessibilityIdentifier("seyal-pane-focus-\(paneLo)")
        placeholder.target = self
        placeholder.action = #selector(requestFocus)
        placeholder.translatesAutoresizingMaskIntoConstraints = false
        if !focused {
            addSubview(placeholder)
            NSLayoutConstraint.activate([
                placeholder.centerXAnchor.constraint(equalTo: centerXAnchor),
                placeholder.centerYAnchor.constraint(equalTo: centerYAnchor),
            ])
        }
        layer?.borderWidth = focused ? 1.5 : 1
        layer?.borderColor = focused
            ? NSColor.controlAccentColor.cgColor
            : NSColor.separatorColor.cgColor
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func installLiveSurface(_ surface: NSView) {
        placeholder.removeFromSuperview()
        subviews.forEach { $0.removeFromSuperview() }
        addSubview(surface)
        NSLayoutConstraint.activate([
            surface.leadingAnchor.constraint(equalTo: leadingAnchor),
            surface.trailingAnchor.constraint(equalTo: trailingAnchor),
            surface.topAnchor.constraint(equalTo: topAnchor),
            surface.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    @objc private func requestFocus() {
        board?.focusPane(lo: paneLo, hi: paneHi)
    }

    override func mouseDown(with event: NSEvent) {
        if !isFocusedLeaf {
            requestFocus()
        } else {
            super.mouseDown(with: event)
        }
    }
}
