import AppKit

enum SeyalDesignTokens {
    enum Layout {
        static let topChromeHeight: CGFloat = 44
        static let leftContextWidth: CGFloat = 220
        static let inspectorWidth: CGFloat = 280
        static let paneCornerRadius: CGFloat = 12
        static let blockCornerRadius: CGFloat = 10
        static let compactSpacing: CGFloat = 8
        static let standardSpacing: CGFloat = 12
        static let panelInset: CGFloat = 12
        static let composerMinHeight: CGFloat = 56
        static let composerMaxPreviewHeight: CGFloat = 132
        static let tabMinWidth: CGFloat = 108
        static let tabMaxWidth: CGFloat = 180
    }

    @MainActor
    enum Typography {
        static let chrome = NSFont.systemFont(ofSize: 12, weight: .medium)
        static let section = NSFont.systemFont(ofSize: 11, weight: .semibold)
        static let body = NSFont.systemFont(ofSize: 12, weight: .regular)
        static let bodyEmphasized = NSFont.systemFont(ofSize: 12, weight: .medium)
        static let metadata = NSFont.systemFont(ofSize: 10, weight: .regular)
        static let command = NSFont.monospacedSystemFont(ofSize: 12, weight: .medium)
        static let terminal = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
    }

    @MainActor
    enum Palette {
        static let windowBackground = NSColor.windowBackgroundColor
        static let chromeBackground = NSColor.controlBackgroundColor
        static let panelBackground = NSColor.controlBackgroundColor
        static let paneBackground = NSColor.textBackgroundColor
        static let elevatedBackground = NSColor.underPageBackgroundColor
        static let separator = NSColor.separatorColor
        static let textPrimary = NSColor.labelColor
        static let textSecondary = NSColor.secondaryLabelColor
        static let textTertiary = NSColor.tertiaryLabelColor
        static let focus = NSColor.controlAccentColor
        static let success = NSColor.systemGreen
        static let warning = NSColor.systemOrange
        static let failure = NSColor.systemRed
    }

    @MainActor
    static func configureRoundedPanel(_ view: NSView, radius: CGFloat = Layout.paneCornerRadius) {
        view.wantsLayer = true
        view.layer?.cornerRadius = radius
        view.layer?.borderWidth = 1
        view.layer?.borderColor = Palette.separator.cgColor
        view.layer?.backgroundColor = Palette.panelBackground.cgColor
    }
}
