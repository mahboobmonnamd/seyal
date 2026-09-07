#!/bin/sh
set -eu

# Research-only runner. It invokes existing production-path tests and writes
# evidence outside target/; it does not add persistence behavior.
output=${1:-experiments/issue-687/runtime-evidence-results.txt}
mkdir -p "$(dirname "$output")"

{
  echo "issue=687-real-runtime-evidence"
  echo "head=$(git rev-parse HEAD)"
  echo "platform=$(uname -a)"
  rustc -Vv
  echo "--- macos_runtime detach/reattach ---"
  cargo test --locked -p seyal-runtime --test macos_runtime \
    headless_detach_preserves_execution_identity_and_terminal_state -- --exact --nocapture
  echo "--- pass8 UDS resync/reattach ---"
  cargo test --locked -p seyal-runtime --test pass8_resync_reattach \
    resync_and_detach_reattach_preserve_execution_block_identity_and_anchor -- --exact --nocapture
  echo "--- pass5 same-execution fanout ---"
  cargo test --locked -p seyal-runtime --test pass5_candidate_d_matrix \
    same_execution_fanout_is_consistent_at_4_8_and_16_viewers -- --exact --nocapture
  echo "--- pass5 stalled-viewer recovery ---"
  cargo test --locked -p seyal-runtime --test pass5_candidate_d_matrix \
    stalled_viewer_is_superseded_and_recovers_from_current_snapshot -- --exact --nocapture
  echo "--- pass10 disconnect cleanup ---"
  cargo test --locked -p seyal-runtime --test pass10_disconnect_during \
    disconnect_during_snapshot_chunking_cleans_up_and_allows_full_reattach -- --exact --nocapture
} 2>&1 | tee "$output"
