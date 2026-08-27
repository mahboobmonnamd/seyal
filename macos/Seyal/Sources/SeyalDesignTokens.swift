import AppKit

enum SeyalDesignTokens {
    enum Layout {
        static let topChromeHeight: CGFloat = 48
        static let leftContextWidth: CGFloat = 236
        static let inspectorWidth: CGFloat = 292
        static let inspectorRailWidth: CGFloat = 36
        static let paneCornerRadius: CGFloat = 12
        static let blockCornerRadius: CGFloat = 10
        static let compactSpacing: CGFloat = 6
        static let standardSpacing: CGFloat = 10
        static let panelInset: CGFloat = 12
        static let composerMinHeight: CGFloat = 58
        static let composerMaxPreviewHeight: CGFloat = 116
        static let tabMinWidth: CGFloat = 118
        static let tabMaxWidth: CGFloat = 190
    }

    @MainActor
    enum Typography {
        static let chrome = NSFont.systemFont(ofSize: 12, weight: .medium)
        static let chromeEmphasized = NSFont.systemFont(ofSize: 12, weight: .semibold)
        static let section = NSFont.systemFont(ofSize: 10, weight: .semibold)
        static let body = NSFont.systemFont(ofSize: 12, weight: .regular)
        static let bodyEmphasized = NSFont.systemFont(ofSize: 12, weight: .semibold)
        static let metadata = NSFont.systemFont(ofSize: 10, weight: .regular)
        static let metadataEmphasized = NSFont.systemFont(ofSize: 10, weight: .medium)
        static let command = NSFont.monospacedSystemFont(ofSize: 12, weight: .semibold)
        static let terminal = NSFont.monospacedSystemFont(ofSize: 11.5, weight: .regular)
    }

    /// The frozen M001 reference is intentionally a dark, low-contrast workspace.
    /// Do not derive these surfaces from the user's current macOS light/dark mode;
    /// product theming will replace this preview palette behind a real theme model.
    ///
    /// These are computed rather than stored NSColor properties. AppKit colors are
    /// MainActor-isolated, and Swift 6 must not perform NSColor construction from a
    /// nonisolated static-storage initializer while compiling either architecture.
    @MainActor
    enum Palette {
        static var windowBackground: NSColor { rgb(12, 15, 20) }
        static var chromeBackground: NSColor { rgb(15, 19, 26) }
        static var panelBackground: NSColor { rgb(16, 21, 29) }
        static var paneBackground: NSColor { rgb(10, 14, 20) }
        static var elevatedBackground: NSColor { rgb(20, 26, 36) }
        static var blockBackground: NSColor { rgb(15, 21, 30) }
        static var blockSelectedBackground: NSColor { rgb(18, 25, 36) }
        static var separator: NSColor { rgb(39, 46, 59) }
        static var subtleSeparator: NSColor { rgb(30, 36, 47) }
        static var textPrimary: NSColor { rgb(231, 234, 240) }
        static var textSecondary: NSColor { rgb(157, 166, 184) }
        static var textTertiary: NSColor { rgb(100, 112, 132) }
        static var focus: NSColor { rgb(128, 92, 246) }
        static var focusSoft: NSColor { rgb(39, 31, 65) }
        static var success: NSColor { rgb(56, 211, 159) }
        static var warning: NSColor { rgb(245, 165, 36) }
        static var failure: NSColor { rgb(249, 112, 102) }
        static var info: NSColor { rgb(94, 160, 255) }

        private static func rgb(_ red: CGFloat, _ green: CGFloat, _ blue: CGFloat) -> NSColor {
            NSColor(
                srgbRed: red / 255.0,
                green: green / 255.0,
                blue: blue / 255.0,
                alpha: 1
            )
        }
    }

    @MainActor
    static func configureRoundedPanel(
        _ view: NSView,
        radius: CGFloat = Layout.paneCornerRadius
    ) {
        configureRoundedPanel(
            view,
            radius: radius,
            background: Palette.panelBackground
        )
    }

    @MainActor
    static func configureRoundedPanel(
        _ view: NSView,
        radius: CGFloat,
        background: NSColor
    ) {
        view.wantsLayer = true
        view.layer?.cornerRadius = radius
        view.layer?.borderWidth = 1
        view.layer?.borderColor = Palette.separator.cgColor
        view.layer?.backgroundColor = background.cgColor
    }
}
