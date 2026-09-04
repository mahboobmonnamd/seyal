# Pass 9 input / accessibility qualification

- **Issue:** #736
- **Exact production head:** `05664dce493abeafa257dddc3c524b11ac74924a`
- **Artifact:** `pass9-input-accessibility-05664dce493a.json`
- **Surface:** production `InteractiveMetalSurfaceView` as `NSTextInputClient`
- **VoiceOver:** Issue #736 discoverability/focus/reconnect recovery + `announcementRequested`; no marked text as transcript

```json
{
  "acceptsFirstResponder" : true,
  "candidateRectFinite" : true,
  "commit" : "05664dce493abeafa257dddc3c524b11ac74924a",
  "deadKeyMarkedThenCommit" : true,
  "imeCancelWithoutTranscript" : true,
  "imeReplacementCommit" : true,
  "markedTextAbsentFromAccessibilityValue" : true,
  "overallPass" : true,
  "pass7InputSelfTest" : true,
  "schema" : "seyal.pass9.input-accessibility.v2",
  "voiceOverAnnouncementAfterReconnect" : true,
  "voiceOverClaim" : "Issue #736 VoiceOver: discoverable\/focusable terminal surface, AX focused tracks first-responder, recoverable after reconnect-style focus cycle with NSAccessibility announcementRequested, no marked\/rejected text exposed as transcript",
  "voiceOverDiscoverableFocusable" : true,
  "voiceOverFocusedTracksFirstResponder" : true,
  "voiceOverNoMarkedTextAsTranscript" : true,
  "voiceOverRecoverableAfterReconnectFocusCycle" : true
}```
