# Pass 9 input / accessibility qualification

- **Issue:** #736
- **Exact production head:** `5f8108ac6ea1464e5645a00770b163aa524ee6b2`
- **Artifact:** `pass9-input-accessibility-5f8108ac6ea1.json`
- **Surface:** production `InteractiveMetalSurfaceView` as `NSTextInputClient`
- **VoiceOver:** SPEC-009 §10 discoverability/focus/reconnect-style recovery; no marked text as transcript

```json
{
  "acceptsFirstResponder" : true,
  "candidateRectFinite" : true,
  "commit" : "5f8108ac6ea1464e5645a00770b163aa524ee6b2",
  "deadKeyMarkedThenCommit" : true,
  "imeCancelWithoutTranscript" : true,
  "imeReplacementCommit" : true,
  "markedTextAbsentFromAccessibilityValue" : true,
  "overallPass" : true,
  "pass7InputSelfTest" : true,
  "schema" : "seyal.pass9.input-accessibility.v1",
  "voiceOverClaim" : "SPEC-009 §10 VoiceOver smoke: discoverable\/focusable terminal surface, AX focused tracks first-responder, recoverable after reconnect-style focus cycle, no marked\/rejected text exposed as transcript",
  "voiceOverDiscoverableFocusable" : true,
  "voiceOverFocusedTracksFirstResponder" : true,
  "voiceOverNoMarkedTextAsTranscript" : true,
  "voiceOverRecoverableAfterReconnectFocusCycle" : true
}```
