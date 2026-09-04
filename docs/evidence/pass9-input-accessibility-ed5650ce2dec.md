# Pass 9 input / accessibility qualification

- **Issue:** #736
- **Exact production head:** `ed5650ce2dec4b278562fe00dcc73e41bc6e227d`
- **Artifact:** `pass9-input-accessibility-ed5650ce2dec.json`
- **Surface:** production `InteractiveMetalSurfaceView` as `NSTextInputClient`
- **VoiceOver:** Issue #736 discoverability/focus/reconnect recovery + `announcementRequested`; no marked text as transcript

```json
{
  "acceptsFirstResponder" : true,
  "candidateRectFinite" : true,
  "commit" : "ed5650ce2dec4b278562fe00dcc73e41bc6e227d",
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
