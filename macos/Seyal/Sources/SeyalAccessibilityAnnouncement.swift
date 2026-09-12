import AppKit
import Foundation

/// Posts VoiceOver-visible announcements through the production AppKit seam.
/// Track C installs a sink to prove Issue #736 announcement validation without
/// requiring system VoiceOver audio capture.
@MainActor
enum SeyalAccessibilityAnnouncement {
  /// Test/qualification sink for posted announcement strings. Production posts
  /// still go through `NSAccessibility.post(.announcementRequested, ...)`.
  static var qualificationSink: ((String) -> Void)?

  static func post(_ message: String, element: Any) {
    let trimmed = message.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return }
    qualificationSink?(trimmed)
    NSAccessibility.post(
      element: element,
      notification: .announcementRequested,
      userInfo: [
        .announcement: trimmed,
        .priority: NSAccessibilityPriorityLevel.medium.rawValue,
      ]
    )
  }
}
