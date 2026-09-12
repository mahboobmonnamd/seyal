import AppKit

/// Maps Rust-resolved tokens onto AppKit. Not a product theme authority.
enum NativeThemeRealization {
    static func apply(to view: NSView, material: NSVisualEffectView, appearance: NSAppearance) {
        let light = appearance.bestMatch(from: [.aqua, .darkAqua]) == .aqua
        let theme = seyal_app_theme(light ? 1 : 0)
        let canvas = color(theme.canvas)
        let text = color(theme.text)
        view.window?.backgroundColor = canvas
        view.appearance = appearance
        view.layer?.backgroundColor = canvas.cgColor
        material.material = light ? .headerView : .sidebar
        material.appearance = appearance
        applyColors(in: view, text: text)
    }

    private static func applyColors(in view: NSView, text: NSColor) {
        if let field = view as? NSTextField {
            field.textColor = text
            field.font = .systemFont(ofSize: NSFont.systemFontSize)
        }
        if let textView = view as? NSTextView {
            textView.textColor = text
            textView.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        }
        for child in view.subviews {
            applyColors(in: child, text: text)
        }
    }

    static func color(_ packed: UInt32) -> NSColor {
        let red = CGFloat((packed >> 24) & 0xff) / 255
        let green = CGFloat((packed >> 16) & 0xff) / 255
        let blue = CGFloat((packed >> 8) & 0xff) / 255
        let alpha = CGFloat(packed & 0xff) / 255
        return NSColor(srgbRed: red, green: green, blue: blue, alpha: alpha)
    }
}
