import Foundation

enum SeyalProductPalette {
    static func color(
        _ role: SeyalColorRole,
        appearance: SeyalResolvedAppearance,
        increaseContrast: Bool
    ) -> SeyalRGBA {
        let base = appearance == .dark ? dark(role) : light(role)
        guard increaseContrast else { return base }
        return contrastAdjusted(base, appearance: appearance, role: role)
    }

    static func dark(_ role: SeyalColorRole) -> SeyalRGBA {
        switch role {
        case .canvas: .srgb(10, 14, 20)
        case .container: .srgb(12, 15, 20)
        case .utilityReceded: .srgb(16, 21, 29)
        case .utilityActive: .srgb(20, 26, 36)
        case .utilityElevated: .srgb(24, 30, 42)
        case .overlay: .srgb(22, 28, 40)
        case .attentionFill: .srgb(42, 28, 24)
        case .textPrimary: .srgb(231, 234, 240)
        case .textSecondary: .srgb(157, 166, 184)
        case .textMuted: .srgb(100, 112, 132)
        case .textAttention: .srgb(249, 180, 140)
        case .seamRest: .srgb(26, 32, 42)
        case .seamHover: .srgb(34, 40, 52)
        case .seamFocus: .srgb(132, 100, 232)
        case .seamRunning: .srgb(245, 165, 36)
        case .seamAttention: .srgb(249, 112, 102)
        case .focus: .srgb(132, 100, 232)
        case .selection: .srgb(33, 28, 57)
        case .success: .srgb(56, 211, 159)
        case .warning: .srgb(245, 165, 36)
        case .danger: .srgb(249, 112, 102)
        case .information: .srgb(94, 160, 255)
        case .agentActivity: .srgb(132, 100, 232)
        case .remoteDegraded: .srgb(245, 165, 36)
        }
    }

    static func light(_ role: SeyalColorRole) -> SeyalRGBA {
        switch role {
        case .canvas: .srgb(252, 252, 250)
        case .container: .srgb(246, 246, 244)
        case .utilityReceded: .srgb(238, 239, 236)
        case .utilityActive: .srgb(232, 234, 230)
        case .utilityElevated: .srgb(255, 255, 255)
        case .overlay: .srgb(255, 255, 255)
        case .attentionFill: .srgb(255, 244, 238)
        case .textPrimary: .srgb(28, 32, 38)
        case .textSecondary: .srgb(90, 98, 110)
        case .textMuted: .srgb(130, 138, 148)
        case .textAttention: .srgb(160, 70, 40)
        case .seamRest: .srgb(220, 222, 218)
        case .seamHover: .srgb(196, 200, 194)
        case .seamFocus: .srgb(92, 70, 180)
        case .seamRunning: .srgb(180, 110, 12)
        case .seamAttention: .srgb(196, 64, 54)
        case .focus: .srgb(92, 70, 180)
        case .selection: .srgb(232, 226, 250)
        case .success: .srgb(20, 140, 100)
        case .warning: .srgb(180, 110, 12)
        case .danger: .srgb(196, 64, 54)
        case .information: .srgb(40, 110, 190)
        case .agentActivity: .srgb(92, 70, 180)
        case .remoteDegraded: .srgb(180, 110, 12)
        }
    }

    private static func contrastAdjusted(
        _ color: SeyalRGBA,
        appearance: SeyalResolvedAppearance,
        role: SeyalColorRole
    ) -> SeyalRGBA {
        switch role {
        case .textPrimary, .textSecondary, .textMuted, .textAttention:
            if appearance == .dark {
                return SeyalRGBA(
                    red: min(color.red + 0.08, 1),
                    green: min(color.green + 0.08, 1),
                    blue: min(color.blue + 0.08, 1),
                    alpha: color.alpha
                )
            }
            return SeyalRGBA(
                red: max(color.red - 0.08, 0),
                green: max(color.green - 0.08, 0),
                blue: max(color.blue - 0.08, 0),
                alpha: color.alpha
            )
        case .seamRest, .seamHover:
            if appearance == .dark {
                return SeyalRGBA(
                    red: min(color.red + 0.12, 1),
                    green: min(color.green + 0.12, 1),
                    blue: min(color.blue + 0.12, 1),
                    alpha: 1
                )
            }
            return SeyalRGBA(
                red: max(color.red - 0.12, 0),
                green: max(color.green - 0.12, 0),
                blue: max(color.blue - 0.12, 0),
                alpha: 1
            )
        default:
            return color
        }
    }
}
