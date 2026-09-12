# M001 UI Design System and Configuration Boundary

**Status:** Pre-migration implementation map; target ownership is governed by ADR-015 and migration by #740  
**Authority:** Subordinate to `SEYAL-ADAPTIVE-DEPTH-DESIGN-LANGUAGE.md`, `SEYAL-UNIVERSAL-COMPONENT-CONTRACT.md`, `SEYAL-ARCH-FOUNDATION-RD-001.md` §13, and R-017–R-020  
**Owning Issue:** #740

## Purpose

Every first-party Seyal UI surface must obtain typography, colour, spacing, sizing, depth, and appearance from one resolved visual configuration.

This document describes the native implementation boundary. It does not replace the design language or authorize screen-by-screen rebuilds.

## Ownership

> **Pre-migration note (ADR-015):** The Swift type names and native resolver described here document the existing implementation, not the target authority. Under ADR-015 and #740, Rust owns portable TOML parsing, defaults, validation, precedence and resolved visual semantics; AppKit only realizes the typed colors, fonts and materials using native APIs.

```text
product tokens (canonical)
        ↓
user TOML  →  typed SeyalUserUISettings
optional Lua ConfigPatch (cold overlay only)
        ↓
SeyalThemeResolver
        ↓
immutable SeyalResolvedVisualConfiguration
        ↓
native AppKit mapping (materials, fonts, seams)
```

- Product tokens are not user-configurable.
- User configuration may override appearance, fonts, padding, utility opacity, and reduced-material preference only.
- Native views consume the snapshot. They must not parse TOML, execute Lua, or store a second palette.

## Terminal boundary

Application typography and terminal typography are resolved separately.

- Terminal fonts feed `TerminalFontResolver` / Metal glyph atlas construction.
- ANSI / shell theme colours remain terminal-owned and are not normalized into Seyal application colours.
- D0 truth surfaces stay opaque. Frost is limited to utility chrome.

Configuration parsing never enters PTY → VT → terminal state → damage → Metal hot paths.

## Lua

Lua is a typed cold overlay (`SeyalConfigPatch`) only. This milestone does **not** embed a Lua VM. A future runtime may produce the same patch at load/reload. It must not mutate views, run per frame, or join terminal I/O.

## Accessibility

Reduced transparency forces tonal/opaque utility materials. Reduced motion zeroes UI transition durations. Increased contrast strengthens text/seam tokens. These overrides apply after user settings.
