import AppKit
import XCTest
@testable import Seyal

@MainActor
final class SeyalDesignSystemTests: XCTestCase {
  func testDefaultTokenResolutionIsDeterministic() {
    let first = SeyalThemeResolver.canonical(.dark)
    let second = SeyalThemeResolver.canonical(.dark)
    XCTAssertEqual(first.appearance, .dark)
    XCTAssertEqual(first.colors[.canvas], second.colors[.canvas])
    XCTAssertEqual(first.metrics, second.metrics)
    XCTAssertTrue(SeyalMetrics.validate(first.metrics))
    XCTAssertEqual(first.terminalFont.family, "Menlo")
    XCTAssertEqual(first.typography[.windowTitle].pointSize, first.settings.uiFontSize)
    XCTAssertEqual(first.typography[.terminal].pointSize, first.settings.terminalFontSize)
  }

  func testLightAndDarkShareHierarchyAndDifferInCanvas() {
    let dark = SeyalThemeResolver.canonical(.dark)
    let light = SeyalThemeResolver.canonical(.light)
    XCTAssertEqual(dark.metrics, light.metrics)
    XCTAssertEqual(dark.typography.specs.keys, light.typography.specs.keys)
    XCTAssertLessThan(dark.colors[.canvas].luminance, 0.2)
    XCTAssertGreaterThan(light.colors[.canvas].luminance, 0.8)
    XCTAssertEqual(dark.material(for: .truth).intent, .opaque)
    XCTAssertEqual(light.material(for: .truth).intent, .opaque)
    XCTAssertNotEqual(dark.colors[.textPrimary], light.colors[.textPrimary])
  }

  func testTOMLOverridesApplyThroughTypedSettings() {
    let toml = """
      [ui]
      appearance = "light"
      window-padding = 12
      utility-opacity = 0.9
      [ui.font]
      family = "Helvetica"
      size = 14
      fallbacks = ["Lucida Grande"]
      [terminal]
      padding = 10
      [terminal.font]
      family = "Menlo"
      size = 16
      """
    let loaded = SeyalUIConfiguration.load(tomlText: toml)
    XCTAssertEqual(loaded.settings.appearance, .light)
    XCTAssertEqual(loaded.settings.windowPadding, 12)
    XCTAssertEqual(loaded.settings.uiFontFamily, "Helvetica")
    XCTAssertEqual(loaded.settings.uiFontSize, 14)
    XCTAssertEqual(loaded.settings.terminalFontSize, 16)
    XCTAssertEqual(loaded.settings.terminalPadding, 10)
    XCTAssertEqual(loaded.source, "toml")

    let visual = SeyalThemeResolver.resolve(
      settings: loaded.settings,
      platformAppearance: .dark
    )
    XCTAssertEqual(visual.appearance, .light)
    XCTAssertEqual(visual.metrics.windowPadding, 12)
    XCTAssertEqual(visual.uiFont.family, "Helvetica")
  }

  func testInvalidTOMLFallsBackWithoutPartialState() {
    let loaded = SeyalUIConfiguration.load(tomlText: "this is not = toml [")
    XCTAssertTrue(loaded.diagnostics.usedFullDefaultFallback)
    XCTAssertEqual(loaded.settings, .default)
  }

  func testInvalidValuesClampOrIgnoreAndKeepCompleteSettings() {
    let toml = """
      [ui]
      appearance = "neon"
      window-padding = 99
      utility-opacity = 0.2
      [ui.font]
      size = "huge"
      """
    let loaded = SeyalUIConfiguration.load(tomlText: toml)
    XCTAssertEqual(loaded.settings.appearance, .system)
    XCTAssertEqual(loaded.settings.windowPadding, 24)
    XCTAssertEqual(loaded.settings.utilityOpacity, 0.85)
    XCTAssertEqual(loaded.settings.uiFontSize, SeyalUserUISettings.default.uiFontSize)
    XCTAssertFalse(loaded.diagnostics.warnings.isEmpty)
    XCTAssertFalse(loaded.diagnostics.usedFullDefaultFallback)
  }

  func testEnvironmentAndLuaOverlayPrecedence() {
    struct Overlay: SeyalColdConfigurationOverlay {
      func configPatch() throws -> SeyalConfigPatch {
        var patch = SeyalConfigPatch.empty
        patch.uiFontSize = 15
        patch.reducedMaterial = true
        return patch
      }
    }

    let toml = """
      [ui]
      appearance = "dark"
      [ui.font]
      size = 11
      """
    let loaded = SeyalUIConfiguration.load(
      tomlText: toml,
      overlay: Overlay(),
      environment: ["SEYAL_UI_APPEARANCE": "light"]
    )
    XCTAssertEqual(loaded.settings.appearance, .light)
    XCTAssertEqual(loaded.settings.uiFontSize, 15)
    XCTAssertTrue(loaded.settings.reducedMaterial)
  }

  func testReducedTransparencyMapsUtilityToTonal() {
    let frosted = SeyalThemeResolver.canonical(.dark)
    let reduced = SeyalThemeResolver.canonical(
      .dark,
      accessibility: SeyalAccessibilitySignals(
        reduceTransparency: true,
        reduceMotion: false,
        increaseContrast: false
      )
    )
    XCTAssertEqual(frosted.material(for: .recededUtility).intent, .frosted)
    XCTAssertEqual(reduced.material(for: .recededUtility).intent, .tonal)
    XCTAssertEqual(reduced.material(for: .truth).intent, .opaque)
    XCTAssertTrue(reduced.reduceTransparency)
  }

  func testReducedMotionDisablesDurations() {
    let motion = SeyalThemeResolver.canonical(
      .light,
      accessibility: SeyalAccessibilitySignals(
        reduceTransparency: false,
        reduceMotion: true,
        increaseContrast: false
      )
    ).motion
    XCTAssertFalse(motion.allowsMotion)
    XCTAssertEqual(motion.focusDuration, 0)
  }

  func testTypographyRolesResolveDistinctSpecs() {
    let visual = SeyalThemeResolver.canonical(.dark)
    XCTAssertNotEqual(
      visual.typography.specs[.windowTitle]?.weight,
      visual.typography.specs[.uiBody]?.weight
    )
    XCTAssertEqual(visual.typography.specs[.terminal]?.family, "Menlo")
    XCTAssertEqual(visual.typography.specs[.composer]?.family, "Menlo")
    XCTAssertNotEqual(visual.typography.specs[.uiBody]?.family, "Menlo")
  }

  func testTerminalFontResolverHonoursConfiguredFamily() {
    let spec = SeyalResolvedFontSpec(family: "Menlo", fallbacks: ["Courier"], pointSize: 14)
    let resolver = TerminalFontResolver(spec: spec)
    XCTAssertEqual(resolver.resolvedFamily, "Menlo")
    XCTAssertTrue(resolver.canResolveScalarDirectly(0x41))
  }

  func testRepresentativeComponentsConsumeTokens() {
    let visual = SeyalThemeResolver.canonical(.dark)
    let block = BlockView(
      presentation: BlockPresentation(
        id: "token", command: "ls", state: .completed, elapsed: "Done",
        timestamp: nil, isSelected: false, actions: []
      ),
      bodyView: NSView(),
      visual: visual
    )
    XCTAssertEqual(block.layer?.cornerRadius, visual.metrics.blockCornerRadius)
    XCTAssertFalse(descendants(of: SeyalSemanticSeamView.self, in: block).isEmpty)

    let composer = PaneComposerShellView(mode: .available, draft: "pwd", visual: visual)
    XCTAssertEqual(composer.layer?.cornerRadius, visual.metrics.composerCornerRadius)
    XCTAssertEqual(
      descendants(of: NSTextView.self, in: composer).first?.font?.pointSize,
      visual.typography[.composer].pointSize
    )
  }

  func testApplyVisualConfigurationRestylesChromeForLightAppearance() {
    let dark = SeyalThemeResolver.canonical(.dark)
    let light = SeyalThemeResolver.canonical(.light)
    let shell = SeyalShellPreviewFactory.make(
      frame: NSRect(x: 0, y: 0, width: 1280, height: 800),
      visual: dark
    )
    shell.layoutSubtreeIfNeeded()
    shell.applyVisualConfiguration(light)
    shell.layoutSubtreeIfNeeded()

    let labels = descendants(of: NSTextField.self, in: shell)
    XCTAssertFalse(labels.isEmpty)
    let primary = light.colors.ns(.textPrimary)
    let matching = labels.contains { field in
      guard let color = field.textColor else { return false }
      return colorsApproximatelyEqual(color, primary)
    }
    XCTAssertTrue(matching, "light apply must restyle chrome text to light primary")
    let bg = shell.layer?.backgroundColor.flatMap { NSColor(cgColor: $0)?.usingColorSpace(.sRGB) }
    let expected = light.colors.ns(.container).usingColorSpace(.sRGB)
    XCTAssertNotNil(bg)
    XCTAssertNotNil(expected)
    if let bg, let expected {
      XCTAssertEqual(bg.redComponent, expected.redComponent, accuracy: 0.02)
      XCTAssertEqual(bg.greenComponent, expected.greenComponent, accuracy: 0.02)
      XCTAssertEqual(bg.blueComponent, expected.blueComponent, accuracy: 0.02)
    }
  }

  func testIncreaseContrastRefreshEmitsUpdatedSnapshot() {
    let controller = SeyalAppearanceController(
      settings: {
        var settings = SeyalUserUISettings.default
        settings.appearance = .dark
        return settings
      }(),
      accessibility: SeyalAccessibilitySignals(
        reduceTransparency: false,
        reduceMotion: false,
        increaseContrast: false
      )
    )
    var received: SeyalResolvedVisualConfiguration?
    controller.onChange = { received = $0 }
    controller.refresh(
      accessibility: SeyalAccessibilitySignals(
        reduceTransparency: false,
        reduceMotion: false,
        increaseContrast: true
      )
    )
    XCTAssertNotNil(received)
    XCTAssertGreaterThan(
      received!.colors[.textPrimary].luminance,
      SeyalThemeResolver.canonical(.dark).colors[.textPrimary].luminance
    )
  }

  func testLuaBoundaryDocumentsColdOverlayOnly() {
    XCTAssertEqual(SeyalLuaConfigurationBoundary.acceptedInput, "SeyalConfigPatch")
    XCTAssertTrue(
      SeyalLuaConfigurationBoundary.forbiddenDomains.contains("Metal rendering")
    )
    XCTAssertTrue(SeyalLuaConfigurationBoundary.runtimeStatus.contains("deferred"))
  }

  private func descendants<T: NSView>(of type: T.Type, in root: NSView) -> [T] {
    var found: [T] = []
    func walk(_ view: NSView) {
      if let match = view as? T { found.append(match) }
      view.subviews.forEach(walk)
    }
    walk(root)
    return found
  }

  private func colorsApproximatelyEqual(_ lhs: NSColor, _ rhs: NSColor) -> Bool {
    guard let a = lhs.usingColorSpace(.sRGB), let b = rhs.usingColorSpace(.sRGB) else {
      return false
    }
    return abs(a.redComponent - b.redComponent) < 0.02
      && abs(a.greenComponent - b.greenComponent) < 0.02
      && abs(a.blueComponent - b.blueComponent) < 0.02
  }
}
