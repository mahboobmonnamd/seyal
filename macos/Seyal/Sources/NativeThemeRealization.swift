import AppKit

/// Maps Rust-resolved tokens onto AppKit. Not a product theme authority.
struct NativeTheme {
    let canvas: NSColor
    let container: NSColor
    let utility: NSColor
    let utilityActive: NSColor
    let elevated: NSColor
    let attentionFill: NSColor
    let overlay: NSColor
    let text: NSColor
    let secondary: NSColor
    let muted: NSColor
    let accent: NSColor
    let seam: NSColor
    let success: NSColor
    let warning: NSColor
    let danger: NSColor
    let appearance: NSAppearance
    let reduceTransparency: Bool
    let reduceMotion: Bool
    let intentReceded: UInt16
    let intentActive: UInt16
    let intentAttention: UInt16
    let focusDuration: CGFloat
    let overlayDuration: CGFloat
}

enum NativeThemeRealization {
    static func accessibilityFlags(for appearance: NSAppearance) -> UInt16 {
        var flags: UInt16 = 0
        if NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency {
            flags |= UInt16(SEYAL_APP_THEME_REDUCE_TRANSPARENCY)
        }
        if NSWorkspace.shared.accessibilityDisplayShouldReduceMotion {
            flags |= UInt16(SEYAL_APP_THEME_REDUCE_MOTION)
        }
        if NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast {
            flags |= UInt16(SEYAL_APP_THEME_INCREASE_CONTRAST)
        }
        // appearance is reserved for future per-window contrast probing
        _ = appearance
        return flags
    }

    static func theme(for appearance: NSAppearance) -> NativeTheme {
        let light = appearance.bestMatch(from: [.aqua, .darkAqua]) == .aqua
        let flags = accessibilityFlags(for: appearance)
        let packed = seyal_app_theme(light ? 1 : 0, flags)
        let canvas = color(packed.canvas)
        let text = color(packed.text)
        let accent = color(packed.accent)
        return NativeTheme(
            canvas: canvas,
            container: color(packed.utility_receded).withAlphaComponent(1),
            utility: color(packed.utility_receded),
            utilityActive: color(packed.utility_active),
            elevated: color(packed.utility_active),
            attentionFill: color(packed.attention_fill),
            overlay: color(packed.overlay),
            text: text,
            secondary: mix(text, canvas, 0.32),
            muted: mix(text, canvas, 0.52),
            accent: accent,
            seam: color(packed.seam),
            success: NSColor(srgbRed: 0.22, green: 0.83, blue: 0.62, alpha: 1),
            warning: NSColor(srgbRed: 0.96, green: 0.65, blue: 0.14, alpha: 1),
            danger: NSColor(srgbRed: 0.98, green: 0.44, blue: 0.40, alpha: 1),
            appearance: light ? NSAppearance(named: .aqua)! : NSAppearance(named: .darkAqua)!,
            reduceTransparency: packed.flags & UInt16(SEYAL_APP_THEME_REDUCE_TRANSPARENCY) != 0,
            reduceMotion: packed.flags & UInt16(SEYAL_APP_THEME_REDUCE_MOTION) != 0,
            intentReceded: packed.intent_receded,
            intentActive: packed.intent_active,
            intentAttention: packed.intent_attention,
            focusDuration: CGFloat(packed.focus_duration),
            overlayDuration: CGFloat(packed.overlay_duration)
        )
    }

    static func chromeSurface(
        _ surface: UInt16,
        appHandle: UInt64,
        appearance: NSAppearance
    ) -> SeyalAppChromeSurface {
        let light = appearance.bestMatch(from: [.aqua, .darkAqua]) == .aqua
        return seyal_app_chrome_surface(
            appHandle,
            surface,
            light ? 1 : 0,
            accessibilityFlags(for: appearance)
        )
    }

    static func color(for surface: SeyalAppChromeSurface, theme: NativeTheme) -> NSColor {
        let packed = color(surface.color)
        if surface.intent == UInt16(SEYAL_APP_MATERIAL_OPAQUE) || theme.reduceTransparency {
            return packed.withAlphaComponent(1)
        }
        return packed
    }

    @MainActor
    static func apply(to view: NSView, material: NSVisualEffectView, appearance: NSAppearance) {
        let theme = theme(for: appearance)
        view.window?.backgroundColor = theme.canvas
        view.window?.appearance = theme.appearance
        view.appearance = theme.appearance
        view.wantsLayer = true
        view.layer?.backgroundColor = theme.canvas.cgColor
        // Root frost stays off; per-region materials are applied by the chrome host.
        material.isHidden = true
        applyColors(in: view, theme: theme)
    }

    @MainActor
    private static func applyColors(in view: NSView, theme: NativeTheme) {
        if let field = view as? NSTextField {
            field.textColor = field.tag == 2 ? theme.muted : (field.tag == 1 ? theme.secondary : theme.text)
            field.backgroundColor = .clear
            field.drawsBackground = false
        }
        if let textView = view as? NSTextView {
            textView.textColor = theme.text
            textView.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
            textView.backgroundColor = .clear
            textView.insertionPointColor = theme.accent
        }
        if let button = view as? NSButton {
            button.appearance = theme.appearance
            button.contentTintColor = theme.text
        }
        for child in view.subviews {
            applyColors(in: child, theme: theme)
        }
    }

    static func color(_ packed: UInt32) -> NSColor {
        let red = CGFloat((packed >> 24) & 0xff) / 255
        let green = CGFloat((packed >> 16) & 0xff) / 255
        let blue = CGFloat((packed >> 8) & 0xff) / 255
        let alpha = CGFloat(packed & 0xff) / 255
        return NSColor(srgbRed: red, green: green, blue: blue, alpha: alpha)
    }

    private static func mix(_ a: NSColor, _ b: NSColor, _ amount: CGFloat) -> NSColor {
        let src = a.usingColorSpace(.sRGB) ?? a
        let dst = b.usingColorSpace(.sRGB) ?? b
        return NSColor(
            srgbRed: src.redComponent + (dst.redComponent - src.redComponent) * amount,
            green: src.greenComponent + (dst.greenComponent - src.greenComponent) * amount,
            blue: src.blueComponent + (dst.blueComponent - src.blueComponent) * amount,
            alpha: 1
        )
    }
}
