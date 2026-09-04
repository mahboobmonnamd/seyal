# Pass 9 input / accessibility qualification

- **Issue:** #736
- **Exact production head:** `21e8e6976c3445ca582bcfe6dd157109cfccdfd1`
- **Artifact:** `pass9-input-accessibility-21e8e6976c34.json`
- **Surface:** production `InteractiveMetalSurfaceView` as `NSTextInputClient`
- **VoiceOver:** discoverability via accessibility role/label/value (system VoiceOver audio not enabled)

```json
{
  "acceptsFirstResponder" : true,
  "candidateRectFinite" : true,
  "commit" : "21e8e6976c3445ca582bcfe6dd157109cfccdfd1",
  "deadKeyMarkedThenCommit" : true,
  "imeCancelWithoutTranscript" : true,
  "imeReplacementCommit" : true,
  "markedTextAbsentFromAccessibilityValue" : true,
  "overallPass" : true,
  "pass7InputSelfTest" : true,
  "schema" : "seyal.pass9.input-accessibility.v1",
  "voiceOverDiscoverableAfterRefresh" : true,
  "voiceOverDiscoverableDisconnected" : true
}```
