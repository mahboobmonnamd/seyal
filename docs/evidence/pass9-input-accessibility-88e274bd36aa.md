# Pass 9 input / accessibility qualification

- **Issue:** #736
- **Exact production head:** `88e274bd36aae78ee6460758fa602692fe78dc38`
- **Artifact:** `pass9-input-accessibility-88e274bd36aa.json`
- **Surface:** production `InteractiveMetalSurfaceView` as `NSTextInputClient`
- **VoiceOver:** discoverability via accessibility role/label/value (system VoiceOver audio not enabled)

```json
{
  "acceptsFirstResponder" : true,
  "candidateRectFinite" : true,
  "commit" : "88e274bd36aae78ee6460758fa602692fe78dc38",
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
