#!/usr/bin/env python3
from pathlib import Path

root = Path(__file__).resolve().parents[1]


def patch(path: str, old: str, new: str) -> None:
    target = root / path
    text = target.read_text()
    if new in text:
        return
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one patch anchor in {path}, found {count}")
    target.write_text(text.replace(old, new, 1))


patch(
    "crates/seyal-runtime/src/runtime/local.rs",
    """                        let Ok(frame) = encode_block_state_frame(&record.to_wire()) else {
                            self.close_local_connection(token);
                            continue;
                        };
                        if !self.send_after_display_frame(token, frame) {
                            continue;
                        }
""",
    """                        let Ok(frame) = encode_block_state_frame(&record.to_wire()) else {
                            self.close_local_connection(token);
                            continue;
                        };
                        #[cfg(feature = \"test-fault-injection\")]
                        if test_fault::take(FaultPoint::BlockCompletionAdmission) {
                            self.close_local_connection(token);
                            continue;
                        }
                        if !self.send_after_display_frame(token, frame) {
                            continue;
                        }
""",
)

path = root / "crates/seyal-runtime/tests/pass8_block_failures.rs"
text = path.read_text()
test = r'''

#[test]
fn completion_admission_failure_disconnects_before_finalized_and_retires_block() {
    let mut harness = Harness::new("admission");
    let execution_id = harness.spawn();
    let mut client = harness.connect();
    harness.attach_until_current(&mut client, execution_id);

    test_fault::fail_next(FaultPoint::BlockCompletionAdmission);
    harness.assert_fails_closed_before_finalized(&mut client);
}
'''
if "completion_admission_failure_disconnects_before_finalized_and_retires_block" not in text:
    path.write_text(text.rstrip() + test)
