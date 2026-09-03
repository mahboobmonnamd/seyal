import AppKit

enum SeyalMaterialPresenter {
    @MainActor
    static func apply(
        _ depth: SeyalDepthLevel,
        to view: NSView,
        visual: SeyalResolvedVisualConfiguration,
        cornerRadius: CGFloat = 0
    ) {
        let material = visual.material(for: depth)
        view.wantsLayer = true
        view.layer?.cornerRadius = cornerRadius
        view.layer?.borderWidth = 0
        view.layer?.masksToBounds = true

        if let effect = view.subviews.compactMap({ $0 as? NSVisualEffectView }).first(
            where: { $0.identifier == NSUserInterfaceItemIdentifier("seyal-material") }
        ) {
            effect.removeFromSuperview()
        }

        switch material.intent {
        case .opaque, .tonal:
            view.layer?.backgroundColor = visual.colors.ns(colorRole(for: depth)).cgColor
        case .frosted:
            let effect = NSVisualEffectView(frame: view.bounds)
            effect.identifier = NSUserInterfaceItemIdentifier("seyal-material")
            effect.autoresizingMask = [.width, .height]
            effect.blendingMode = .withinWindow
            effect.state = .followsWindowActiveState
            effect.material = visualEffectMaterial(for: depth)
            effect.wantsLayer = true
            effect.layer?.cornerRadius = cornerRadius
            view.addSubview(effect, positioned: .below, relativeTo: nil)
            view.layer?.backgroundColor = material.color.alpha < 1
                ? visual.colors.ns(colorRole(for: depth)).withAlphaComponent(CGFloat(material.color.alpha)).cgColor
                : visual.colors.ns(colorRole(for: depth)).cgColor
        }
    }

    private static func colorRole(for depth: SeyalDepthLevel) -> SeyalColorRole {
        switch depth {
        case .truth: .canvas
        case .recededUtility: .utilityReceded
        case .activeUtility: .utilityActive
        case .attention: .attentionFill
        }
    }

    private static func visualEffectMaterial(for depth: SeyalDepthLevel) -> NSVisualEffectView.Material {
        switch depth {
        case .truth: .contentBackground
        case .recededUtility: .sidebar
        case .activeUtility: .menu
        case .attention: .hudWindow
        }
    }
}

@MainActor
final class SeyalSemanticSeamView: NSView {
    private var visual: SeyalResolvedVisualConfiguration
    private var role: SeyalSeamRole

    init(visual: SeyalResolvedVisualConfiguration, role: SeyalSeamRole = .rest) {
        self.visual = visual
        self.role = role
        super.init(frame: .zero)
        translatesAutoresizingMaskIntoConstraints = false
        wantsLayer = true
        setAccessibilityIdentifier("semantic-seam")
        apply()
        heightAnchor.constraint(equalToConstant: visual.metrics.seamWidth).isActive = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("SeyalSemanticSeamView is programmatic")
    }

    func apply(visual: SeyalResolvedVisualConfiguration? = nil, role: SeyalSeamRole? = nil) {
        if let visual { self.visual = visual }
        if let role { self.role = role }
        apply()
    }

    private func apply() {
        layer?.backgroundColor = visual.seamColor(role).cgColor
        layer?.cornerRadius = 0
    }
}

enum SeyalFocusTreatment {
    @MainActor
    static func apply(
        _ focused: Bool,
        to view: NSView,
        visual: SeyalResolvedVisualConfiguration
    ) {
        view.wantsLayer = true
        view.layer?.borderWidth = focused ? visual.metrics.seamWidth : 0
        view.layer?.borderColor = focused
            ? visual.colors.cg(.seamFocus)
            : visual.colors.cg(.seamRest)
        view.layer?.cornerRadius = 0
    }
}

@MainActor
final class SeyalAppearanceController: NSObject {
    private(set) var snapshot: SeyalResolvedVisualConfiguration
    private var settings: SeyalUserUISettings
    private var diagnostics: SeyalConfigurationDiagnostics
    var onChange: ((SeyalResolvedVisualConfiguration) -> Void)?
    private var observers: [NSObjectProtocol] = []

    init(
        settings: SeyalUserUISettings,
        diagnostics: SeyalConfigurationDiagnostics = SeyalConfigurationDiagnostics(),
        accessibility: SeyalAccessibilitySignals = SeyalAppearanceController.currentAccessibility()
    ) {
        self.settings = settings
        self.diagnostics = diagnostics
        snapshot = SeyalThemeResolver.resolve(
            settings: settings,
            platformAppearance: SeyalAppearanceController.platformAppearance(),
            accessibility: accessibility,
            diagnostics: diagnostics
        )
        super.init()
        observePlatformAppearance()
    }

    func applyPlatformAppearanceIfNeeded() {
        refresh(accessibility: Self.currentAccessibility())
    }

    func refresh(accessibility: SeyalAccessibilitySignals) {
        let next = SeyalThemeResolver.resolve(
            settings: settings,
            platformAppearance: Self.platformAppearance(),
            accessibility: accessibility,
            diagnostics: diagnostics
        )
        guard next.appearance != snapshot.appearance
            || next.reduceTransparency != snapshot.reduceTransparency
            || next.motion != snapshot.motion
            || next.settings != snapshot.settings
        else { return }
        snapshot = next
        onChange?(next)
    }

    static func platformAppearance() -> SeyalResolvedAppearance {
        let appearance = NSApp.effectiveAppearance
        let name = appearance.bestMatch(from: [.darkAqua, .aqua])
        return name == .darkAqua ? .dark : .light
    }

    static func currentAccessibility() -> SeyalAccessibilitySignals {
        SeyalAccessibilitySignals(
            reduceTransparency: NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency,
            reduceMotion: NSWorkspace.shared.accessibilityDisplayShouldReduceMotion,
            increaseContrast: NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast
        )
    }

    private func observePlatformAppearance() {
        let center = NotificationCenter.default
        observers.append(center.addObserver(
            forName: Notification.Name("NSApplicationDidChangeEffectiveAppearanceNotification"),
            object: NSApp,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.applyPlatformAppearanceIfNeeded()
            }
        })
        observers.append(center.addObserver(
            forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.applyPlatformAppearanceIfNeeded()
            }
        })
    }
}
