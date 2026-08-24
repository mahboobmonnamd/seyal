import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private var window: NSWindow?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let contentRect = NSRect(x: 0, y: 0, width: 1280, height: 860)
        let shouldUseDesignPreview = CommandLine.arguments.contains("--design-preview")
            || ProcessInfo.processInfo.environment["SEYAL_DESIGN_PREVIEW"] == "1"

        let window = NSWindow(
            contentRect: contentRect,
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )

        if shouldUseDesignPreview {
            window.title = "Seyal — Approved UI Review"
            window.contentView = DesignPreviewRootView(frame: contentRect)
            window.minSize = NSSize(width: 980, height: 720)
        } else {
            window.title = "Seyal"
            window.contentView = MetalSurfaceView(frame: contentRect)
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
private final class DesignPreviewRootView: NSView {
    private enum State: Int, CaseIterable {
        case composer
        case workspace
        case components

        var title: String {
            switch self {
            case .composer:
                "Composer"
            case .workspace:
                "Workspace"
            case .components:
                "Component board"
            }
        }

        var resourceName: String {
            switch self {
            case .composer:
                "UI-REF-001-MULTILINE-COMPOSER"
            case .workspace:
                "UI-REF-002-APPROVED-LIGHT-WORKSPACE"
            case .components:
                "UI-REF-003-APPROVED-LIGHT-COMPONENT-BOARD"
            }
        }

        var summary: String {
            switch self {
            case .composer:
                "Approved dark-mode multiline composer board used for exact visual review."
            case .workspace:
                "Approved light workspace composition with per-pane composers, tabs, agents, and utility surfaces."
            case .components:
                "Approved component reference board used to verify tab, pane, block, composer, and popover details."
            }
        }

        var backgroundColor: NSColor {
            switch self {
            case .composer:
                NSColor(deviceRed: 0.03, green: 0.05, blue: 0.10, alpha: 1.0)
            case .workspace, .components:
                NSColor(deviceRed: 0.97, green: 0.97, blue: 0.98, alpha: 1.0)
            }
        }
    }

    private let titleField = NSTextField(labelWithString: "Approved design review harness")
    private let subtitleField = NSTextField(labelWithString: "")
    private let selector = NSSegmentedControl(labels: State.allCases.map(\.title), trackingMode: .selectOne, target: nil, action: nil)
    private let imageContainer = NSView()
    private let imageView = NSImageView()
    private let footerField = NSTextField(labelWithString: "Preview-only visual authority. This target intentionally shows the approved PNG references instead of invented placeholder chrome.")
    private var currentState: State = .workspace

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true
        translatesAutoresizingMaskIntoConstraints = false
        buildUI()
        applyState(.workspace)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func buildUI() {
        titleField.font = .systemFont(ofSize: 16, weight: .semibold)
        subtitleField.font = .systemFont(ofSize: 12, weight: .regular)
        subtitleField.textColor = .secondaryLabelColor
        subtitleField.lineBreakMode = .byWordWrapping
        subtitleField.maximumNumberOfLines = 2

        selector.segmentStyle = .capsule
        selector.selectedSegment = State.workspace.rawValue
        selector.target = self
        selector.action = #selector(handleSelectionChanged(_:))
        selector.setAccessibilityLabel("Approved design preview state selector")

        imageContainer.wantsLayer = true
        imageContainer.layer?.cornerRadius = 16
        imageContainer.layer?.borderWidth = 1
        imageContainer.layer?.borderColor = NSColor.separatorColor.cgColor

        imageView.imageScaling = .scaleProportionallyUpOrDown
        imageView.imageAlignment = .alignCenter
        imageView.translatesAutoresizingMaskIntoConstraints = false
        imageContainer.addSubview(imageView)

        footerField.font = .systemFont(ofSize: 11, weight: .regular)
        footerField.textColor = .secondaryLabelColor
        footerField.lineBreakMode = .byWordWrapping
        footerField.maximumNumberOfLines = 2

        let stack = NSStackView(views: [titleField, subtitleField, selector, imageContainer, footerField])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 12
        stack.edgeInsets = NSEdgeInsets(top: 18, left: 18, bottom: 18, right: 18)
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)

        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),

            selector.widthAnchor.constraint(greaterThanOrEqualToConstant: 360),

            imageContainer.leadingAnchor.constraint(equalTo: stack.leadingAnchor),
            imageContainer.trailingAnchor.constraint(equalTo: stack.trailingAnchor),
            imageContainer.heightAnchor.constraint(greaterThanOrEqualToConstant: 620),

            imageView.leadingAnchor.constraint(equalTo: imageContainer.leadingAnchor, constant: 12),
            imageView.trailingAnchor.constraint(equalTo: imageContainer.trailingAnchor, constant: -12),
            imageView.topAnchor.constraint(equalTo: imageContainer.topAnchor, constant: 12),
            imageView.bottomAnchor.constraint(equalTo: imageContainer.bottomAnchor, constant: -12),
        ])
    }

    @objc
    private func handleSelectionChanged(_ sender: NSSegmentedControl) {
        guard let state = State(rawValue: sender.selectedSegment) else {
            return
        }
        applyState(state)
    }

    private func applyState(_ state: State) {
        currentState = state
        layer?.backgroundColor = state.backgroundColor.cgColor
        imageContainer.layer?.backgroundColor = state == .composer
            ? NSColor(deviceRed: 0.05, green: 0.07, blue: 0.12, alpha: 1.0).cgColor
            : NSColor.white.cgColor
        subtitleField.stringValue = state.summary
        footerField.stringValue = footerText(for: state)
        imageView.image = loadReferenceImage(named: state.resourceName)
        imageView.setAccessibilityLabel(state.summary)
    }

    private func footerText(for state: State) -> String {
        switch state {
        case .composer:
            "UI-REF-001 is bundled as a PNG so the design-review app can use the exact approved geometry while the native component pass is refined."
        case .workspace:
            "UI-REF-002 remains the authority for pane-local composers, tab sizing, workspace navigation, agent attention, and the right utility surface."
        case .components:
            "UI-REF-003 is used as the component inventory baseline for future AppKit decomposition and screenshot regression."
        }
    }

    private func loadReferenceImage(named resourceName: String) -> NSImage? {
        guard let url = Bundle.main.url(forResource: resourceName, withExtension: "png") else {
            subtitleField.stringValue = "Missing bundled reference image: \(resourceName).png"
            return nil
        }
        return NSImage(contentsOf: url)
    }
}
