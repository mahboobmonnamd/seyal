#!/usr/bin/env python3
from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label} anchor count={count}")
    path.write_text(text.replace(old, new, 1))


surface = Path("macos/Seyal/Sources/MetalSurfaceView.swift")
replace_once(
    surface,
    '''    bridgeReconnectTimer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: false) {\n      [weak self] _ in\n      guard let self, generation == self.bridgeReconnectGeneration else { return }\n      self.bridgeReconnectTimer = nil\n      guard self.shouldRender, self.bridge?.isConnected == false else { return }\n      if self.bridge?.start() == true {\n        self.cancelBridgeReconnect()\n      } else {\n        self.scheduleBridgeReconnect()\n      }\n    }\n  }\n\n  private func cancelBridgeReconnect() {\n''',
    '''    bridgeReconnectTimer = Timer.scheduledTimer(\n      timeInterval: 0.1,\n      target: self,\n      selector: #selector(bridgeReconnectTimerFired(_:)),\n      userInfo: generation,\n      repeats: false\n    )\n  }\n\n  @objc private func bridgeReconnectTimerFired(_ timer: Timer) {\n    guard let generation = timer.userInfo as? UInt64,\n          generation == bridgeReconnectGeneration\n    else { return }\n    bridgeReconnectTimer = nil\n    guard shouldRender, bridge?.isConnected == false else { return }\n    if bridge?.start() == true {\n      cancelBridgeReconnect()\n    } else {\n      scheduleBridgeReconnect()\n    }\n  }\n\n  private func cancelBridgeReconnect() {\n''',
    "main-actor bridge reconnect timer",
)

renderer = Path("macos/Seyal/Sources/RendererValidation.swift")
replace_once(
    renderer,
    '''    deinit {\n        link.delegate = nil\n        link.invalidate()\n    }\n''',
    '''    func invalidate() {\n        link.delegate = nil\n        link.invalidate()\n    }\n''',
    "display-link cleanup",
)
replace_once(
    renderer,
    '''            let displayLinkDriver = DisplayLinkBenchmarkDriver(\n                renderer: renderer,\n                layer: presentationLayer\n            )\n            var preparationSamples = [UInt64]()\n''',
    '''            let displayLinkDriver = DisplayLinkBenchmarkDriver(\n                renderer: renderer,\n                layer: presentationLayer\n            )\n            defer { displayLinkDriver.invalidate() }\n            var preparationSamples = [UInt64]()\n''',
    "display-link cleanup defer",
)
