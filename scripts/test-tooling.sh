#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
  printf '[seyal tooling test] FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -f rust-toolchain.toml ]] || fail "rust-toolchain.toml is missing"
grep -Eq 'channel[[:space:]]*=[[:space:]]*"1\.98\.0"' rust-toolchain.toml || fail "Rust channel is not pinned to 1.98.0"
grep -Eq 'components[[:space:]]*=[[:space:]]*\[[^]]*"rustfmt"' rust-toolchain.toml || fail "rustfmt is not pinned"
grep -Eq 'components[[:space:]]*=[[:space:]]*\[[^]]*"clippy"' rust-toolchain.toml || fail "clippy is not pinned"

[[ -f scripts/check-pass9-calibration-coverage.py ]] || fail "Pass 9 calibration coverage validator is missing"
python3 scripts/check-pass9-calibration-coverage.py --self-test >/dev/null || fail "Pass 9 calibration coverage validator self-test failed"
grep -q -- '--controlled-calibration' crates/seyal-runtime/benches/pass9_preimplementation_calibration.rs || fail "Pass 9 calibration binary lacks explicit invocation gate"
grep -q -- '--metrics-self-test' crates/seyal-runtime/benches/pass9_preimplementation_calibration.rs || fail "Pass 9 calibration binary lacks metrics integrity self-test"
grep -q -- '-- --controlled-calibration' scripts/task.sh || fail "Pass 9 calibration task lacks the explicit invocation argument"
grep -q -- '-- --metrics-self-test' scripts/task.sh || fail "Pass 9 calibration task does not run metrics integrity self-test"

pass9_metrics='crates/seyal-runtime/benches/pass9_preimplementation_calibration/metrics.rs'
pass9_worker='crates/seyal-runtime/benches/pass9_preimplementation_calibration/worker.rs'
grep -q 'libc::proc_pidinfo' "$pass9_metrics" || fail "Pass 9 metrics must query macOS process state without spawning ps"
grep -q 'PROC_PIDTASKINFO' "$pass9_metrics" || fail "Pass 9 metrics must read target RSS and thread count from proc_taskinfo"
grep -q 'PROC_PIDLISTFDS' "$pass9_metrics" || fail "Pass 9 metrics must count the target process FDs"
if grep -q 'Command::new("/bin/ps")' "$pass9_metrics"; then
  fail "Pass 9 metrics must not perturb measured processes by spawning ps"
fi
if grep -q 'read_dir("/dev/fd")' "$pass9_metrics"; then
  fail "Pass 9 metrics must not count the sampler process FDs"
fi
grep -q 'sync_channel::<WorkerCommand>' "$pass9_worker" || fail "Pass 9 Runtime diagnostics must use a preallocated bounded command channel"
grep -q 'read_line(&mut command_buffer)' "$pass9_worker" || fail "Pass 9 Runtime diagnostics must reuse their command input buffer"
if grep -q 'mpsc::channel::<String>' "$pass9_worker"; then
  fail "Pass 9 Runtime diagnostics must not allocate a String channel node per sample"
fi
if grep -q 'stdin.lock().lines()' "$pass9_worker"; then
  fail "Pass 9 Runtime diagnostics must not allocate a fresh command String per sample"
fi
if grep -q 'line.split.*collect::<Vec' "$pass9_worker"; then
  fail "Pass 9 Runtime diagnostic parsing must not allocate a field vector per sample"
fi

for target in bootstrap bootstrap-agents build test check bench; do
  make -n "$target" >/dev/null || fail "canonical make target '${target}' does not resolve"
done

[[ -f scripts/bootstrap-dev.sh ]] || fail "agent bootstrap script is missing"
[[ -f docs/engineering/AGENT-TOOLING.md ]] || fail "agent tooling policy is missing"
grep -q 'XCODEBUILD_MCP_VERSION=' scripts/bootstrap-dev.sh || fail "XcodeBuildMCP is not pinned"
grep -q 'AI_SDLC_REPO=' scripts/bootstrap-dev.sh || fail "AI-SDLC repository is not declared"
grep -Eq 'AI_SDLC_COMMIT="[0-9a-f]{40}"' scripts/bootstrap-dev.sh || fail "AI-SDLC must be pinned by full commit SHA"
grep -q 'AI_SDLC_COMMIT="105e0cedc392a4468308d9bbfd6c273ad44924fe"' scripts/bootstrap-dev.sh || fail "AI-SDLC pin must include merged generic pr-review"
grep -q '^AI_SDLC_SKILLS=(' scripts/bootstrap-dev.sh || fail "AI-SDLC skill manifest is missing"
grep -q '^ensure_ai_sdlc()' scripts/bootstrap-dev.sh || fail "AI-SDLC materialization is missing"
for generic_skill in project-context development-readiness work-item-design implementation code-review verification pr-review; do
  grep -q "  ${generic_skill}$" scripts/bootstrap-dev.sh || fail "AI-SDLC generic skill is not pinned: ${generic_skill}"
done
grep -q 'tools/project_context.py' scripts/bootstrap-dev.sh || fail "AI-SDLC project-context tool verification is missing"
grep -q 'project_context.py.*--root.*validate' scripts/bootstrap-dev.sh || fail "agent bootstrap must validate the derived context index"
grep -q 'github-mcp-server' scripts/bootstrap-dev.sh || fail "GitHub MCP bootstrap is missing"
grep -q 'mcpbridge' scripts/bootstrap-dev.sh || fail "official Xcode MCP bootstrap is missing"
grep -q 'xcodebuildmcp@${XCODEBUILD_MCP_VERSION}' scripts/bootstrap-dev.sh || fail "XcodeBuildMCP configuration is missing"
grep -q '^configure_copilot()' scripts/bootstrap-dev.sh || fail "GitHub Copilot MCP setup is missing"
grep -q 'configure_mcp_client copilot "GitHub Copilot CLI" builtin' scripts/bootstrap-dev.sh || fail "Copilot must use built-in GitHub MCP mode"
grep -q '^configure_cursor()' scripts/bootstrap-dev.sh || fail "Cursor MCP setup is missing"
grep -q 'SEYAL_CURSOR_MCP_CONFIG' scripts/bootstrap-dev.sh || fail "Cursor MCP config path is missing"
grep -q 'servers\["xcode"\]' scripts/bootstrap-dev.sh || fail "Cursor Xcode MCP setup is missing"
grep -q 'servers\["xcodebuild"\]' scripts/bootstrap-dev.sh || fail "Cursor XcodeBuildMCP setup is missing"
grep -q 'if has claude || has codex || has cursor; then' scripts/bootstrap-dev.sh || fail "external GitHub MCP should only be provisioned for clients that need it"

for adapter in project-context development-readiness verification code-review; do
  [[ -f ".agents/skills/${adapter}/SKILL.md" ]] || fail "Seyal ${adapter} adapter is missing"
  [[ -f ".claude/skills/${adapter}/SKILL.md" ]] || fail "Claude ${adapter} adapter is missing"
done

[[ -f .agents/skills/pr-review/SKILL.md ]] || fail "Seyal pr-review facade is missing"
[[ -f .claude/skills/pr-review/SKILL.md ]] || fail "Claude pr-review adapter is missing"

grep -q '.sdlc/framework/skills/work-item-design/SKILL.md' .agents/skills/issue-refinement/SKILL.md || fail "issue-refinement must delegate to AI-SDLC work-item-design"
grep -q '.sdlc/framework/skills/implementation/SKILL.md' .agents/skills/implement-issue/SKILL.md || fail "implement-issue must delegate to AI-SDLC implementation"
grep -q '.sdlc/framework/skills/code-review/SKILL.md' .agents/skills/code-review/SKILL.md || fail "code-review must delegate to AI-SDLC code-review"
grep -q '.sdlc/framework/skills/pr-review/SKILL.md' .agents/skills/pr-review/SKILL.md || fail "pr-review must delegate to AI-SDLC pr-review"
if grep -q '.sdlc/framework/skills/code-review/SKILL.md' .agents/skills/pr-review/SKILL.md; then
  fail "pr-review must not regress to the focused AI-SDLC code-review authority"
fi
grep -q '.sdlc/framework/skills/verification/SKILL.md' .agents/skills/milestone-validation/SKILL.md || fail "milestone-validation must build on AI-SDLC verification"
grep -q '.sdlc/framework/skills/development-readiness/SKILL.md' .agents/skills/development-readiness/SKILL.md || fail "development-readiness adapter must delegate to AI-SDLC"
grep -q '.sdlc/framework/skills/verification/SKILL.md' .agents/skills/verification/SKILL.md || fail "verification adapter must delegate to AI-SDLC"

[[ -f .sdlc/context/_meta.yaml ]] || fail "Seyal SDLC context metadata is missing"
[[ -f .sdlc/graph/context-index.json ]] || fail "Seyal derived context index is missing"
python3 -m json.tool .sdlc/graph/context-index.json >/dev/null || fail "Seyal context index is not valid JSON"
python3 <<'PY' || fail "Seyal context index source fingerprints are stale"
import hashlib
import json
from pathlib import Path

root = Path('.')
with (root / '.sdlc/graph/context-index.json').open(encoding='utf-8') as handle:
    index = json.load(handle)

errors = []
for node in index.get('nodes', []):
    node_id = node.get('id', '<unknown>')
    for source in node.get('sources', []):
        rel = source.get('path')
        fingerprint = source.get('fingerprint')
        if isinstance(fingerprint, dict):
            expected = fingerprint.get('value')
        else:
            expected = fingerprint
        if not isinstance(rel, str) or not isinstance(expected, str):
            errors.append(f'{node_id}: malformed source fingerprint')
            continue
        path = root / rel
        if not path.is_file():
            errors.append(f'{node_id}: missing source {rel}')
            continue
        data = path.read_bytes()
        header = f'blob {len(data)}\0'.encode('utf-8')
        actual = hashlib.sha1(header + data).hexdigest()
        if actual != expected:
            errors.append(
                f'{node_id}: stale source {rel}: index={expected} current={actual}'
            )

if errors:
    for error in errors:
        print(f'[seyal tooling test] {error}')
    raise SystemExit(1)
PY
[[ ! -e scripts/project_context.py ]] || fail "generic project-context implementation must not be duplicated in Seyal"
grep -q '^/.sdlc/framework/' .gitignore || fail "materialized AI-SDLC framework must remain untracked"

ai_sdlc_commit="$(sed -n 's/^AI_SDLC_COMMIT="\([0-9a-f]\{40\}\)"$/\1/p' scripts/bootstrap-dev.sh)"
[[ -n "$ai_sdlc_commit" ]] || fail "could not read AI-SDLC pin"
grep -q "pinned_revision: \"${ai_sdlc_commit}\"" .sdlc/context/_meta.yaml || fail "SDLC metadata pin does not match bootstrap pin"
python3 - "$ai_sdlc_commit" <<'PY' || fail "context index pin does not match bootstrap pin"
import json
import sys

with open('.sdlc/graph/context-index.json', encoding='utf-8') as handle:
    value = json.load(handle)
if value.get('framework', {}).get('pinned_revision') != sys.argv[1]:
    raise SystemExit(1)
PY

# Copilot project skills are loaded natively from .agents/skills; do not create a
# second Copilot-specific project skill tree that can diverge from canonical skills.
if [[ -d .copilot/skills || -d .github/skills ]]; then
  fail "duplicate Copilot project skill adapter tree detected"
fi

tooling_scope=(scripts/bootstrap-dev.sh docs/engineering/AGENT-TOOLING.md)
for forbidden in \
  'frontend-design' \
  'anthropics/skills' \
  'playwright' \
  'AppleDeepDocs' \
  'appledeepdoc' \
  'apple-deep-docs' \
  'SEYAL_ENABLE_APPLE_DEEP_DOCS'; do
  if grep -Fqi "$forbidden" "${tooling_scope[@]}"; then
    fail "non-project tooling returned to Seyal bootstrap/policy: ${forbidden}"
  fi
done

missing="$(mktemp)"
trap 'rm -f "$missing"' EXIT
if SEYAL_RUSTUP="${ROOT}/.definitely-missing-rustup" bash scripts/check-toolchain.sh >"$missing" 2>&1; then
  fail "missing rustup condition unexpectedly succeeded"
fi
grep -q 'rustup is required' "$missing" || fail "missing rustup failure is not actionable"

printf '[seyal tooling test] deterministic task/toolchain metadata tests passed.\n'
