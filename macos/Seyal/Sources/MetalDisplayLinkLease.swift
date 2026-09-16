import QuartzCore

/// Owns the run-loop registration independently of the view's actor-isolated
/// lifetime. Releasing a surface therefore also invalidates its display link.
final class MetalDisplayLinkLease {
  let link: CAMetalDisplayLink

  init(layer: CAMetalLayer) {
    link = CAMetalDisplayLink(metalLayer: layer)
  }

  deinit {
    link.delegate = nil
    link.invalidate()
  }
}
