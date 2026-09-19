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

for target in bootstrap bootstrap-agents build test check bench; do
  make -n "$target" >/dev/null || fail "canonical make target '${target}' does not resolve"
done

[[ -f scripts/bootstrap-dev.sh ]] || fail "agent bootstrap script is missing"
[[ -f docs/engineering/AGENT-TOOLING.md ]] || fail "agent tooling policy is missing"
grep -q 'XCODEBUILD_MCP_VERSION=' scripts/bootstrap-dev.sh || fail "XcodeBuildMCP is not pinned"
grep -q 'AI_SDLC_REPO=' scripts/bootstrap-dev.sh || fail "AI-SDLC repository is not declared"
grep -Eq 'AI_SDLC_COMMIT="[0-9a-f]{40}"' scripts/bootstrap-dev.sh || fail "AI-SDLC must be pinned by full commit SHA"
grep -q 'AI_SDLC_COMMIT="8d329477e41f00e82435fe47d49cfedd724aefc5"' scripts/bootstrap-dev.sh || fail "AI-SDLC pin must include merged working-loop revision"
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

# Parallel-development safety: Ready issues stay unassigned, and the exact
# remote issue branch is the atomic claim before any worktree/production edit.
claim_skill=.agents/skills/implement-issue/SKILL.md
grep -Fq 'mandatory entrypoint for production implementation of a Seyal GitHub Issue' "$claim_skill" || fail "implement-issue must be the mandatory production entrypoint"
grep -Fq 'gh api user --jq .login' "$claim_skill" || fail "implement-issue must resolve authenticated GitHub identity with gh"
grep -Fq 'never set, clear, or change GitHub assignees' "$claim_skill" || fail "implement-issue must leave assignee fields unchanged"
grep -Fq 'its body `State` field explicitly to be Ready' "$claim_skill" || fail "implement-issue must require the Issue State field to be Ready"
grep -Fq 'No linked Project item is required.' "$claim_skill" || fail "implement-issue must not require a Project item"
grep -Fq 'POST /repos/{owner}/{repo}/git/refs' "$claim_skill" || fail "implement-issue must use atomic remote ref creation"
grep -Fq 'The first unambiguous successful creation is the only winner.' "$claim_skill" || fail "implement-issue must make one branch creator the race winner"
grep -Fq 'An existing ref, failed creation, competing creation, or ambiguous response means stop' "$claim_skill" || fail "implement-issue must fail closed on branch reservation conflicts"
grep -Fq 'claimant, exact branch, base SHA, and a link to the confirmed plan comment' "$claim_skill" || fail "implement-issue must record auditable claim details"
grep -Fq 'Issue body/comments are the durable plan and confirmation record; chat is not a prerequisite.' "$claim_skill" || fail "implement-issue must store plan confirmation on the Issue"
grep -Fq 'Claims never expire automatically.' "$claim_skill" || fail "implement-issue must not expire claims automatically"
grep -Fq "A planning parent's assignee does not lock an independent Ready child." "$claim_skill" || fail "implement-issue must allow independent Ready child claims"
refine_skill=.agents/skills/issue-refinement/SKILL.md
grep -Fq 'recommend GitHub sub-issues (one per slice)' "$refine_skill" || fail "issue-refinement must recommend one GitHub sub-issue per slice"
grep -Fq 'Leave new implementation Issues unassigned.' "$refine_skill" || fail "issue-refinement must leave implementation Issues unassigned"
grep -Fq 'a planning parent is not an exclusive claim surface' "$refine_skill" || fail "issue-refinement must allow independent child claims"
grep -Fq 'Any request to **implement, fix, finish, code, or complete a specific GitHub Issue** must enter through' AGENTS.md || fail "AGENTS.md must route implementation requests through implement-issue"
grep -Fq 'one atomically created deterministic issue/<number> branch' docs/engineering/DEVELOPMENT.md || fail "development workflow must use an atomic deterministic issue branch"
grep -Fq 'successful atomic creation of the exact remote `issue/<number>` ref is the exclusive race lock' docs/engineering/DEVELOPMENT.md || fail "development workflow must name the branch as the race lock"
grep -Fq 'Record the execution plan as an Issue comment before reserving a branch.' docs/engineering/DEVELOPMENT.md || fail "development workflow must keep plans in GitHub"
grep -Fq 'No linked Project item is required.' docs/engineering/DEVELOPMENT.md || fail "development workflow must not require a Project item"
if grep -Fq '→ issue/<number>-<short-name>' docs/engineering/DEVELOPMENT.md; then
  fail "new development workflow must not retain the legacy non-deterministic branch convention"
fi
grep -Fq 'GitHub assignees are never used as a lock' docs/engineering/ISSUE-PROTOCOL.md || fail "Issue protocol must use branch reservation instead of assignee lock"
grep -Fq 'exact remote `issue/<number>` Git ref is the atomic claim' docs/engineering/ISSUE-PROTOCOL.md || fail "Issue protocol must define the exact branch claim"
grep -Fq 'There is no automatic claim expiry.' docs/engineering/ISSUE-PROTOCOL.md || fail "Issue protocol must not expire claims automatically"
grep -Fq 'No linked Project item is required.' docs/engineering/ISSUE-PROTOCOL.md || fail "Issue protocol must not require a Project item"
grep -Fq 'body `State` field is explicitly Ready' docs/engineering/ISSUE-PROTOCOL.md || fail "Issue protocol must gate pickup on the Issue State field"
grep -Fq "Set the Issue body's" .agents/skills/issue-refinement/SKILL.md || fail "issue-refinement must set Ready on the Issue body"
grep -Fq "A parent Issue's assignee or planning branch does not automatically lock an independent child." docs/engineering/ISSUE-PROTOCOL.md || fail "Issue protocol must permit non-overlapping child work"
grep -Fq 'Issue-comment plan' site/src/content/docs/developer/index.mdx || fail "Developer Guide must put implementation plans on GitHub Issues"
grep -Fq 'atomic creation of exact remote issue/<number>' site/src/content/docs/developer/index.mdx || fail "Developer Guide must describe the atomic branch claim"
if grep -Eiq 'exclusive assignee claim|sole Issue assignee|reassign the Issue' site/src/content/docs/developer/index.mdx; then
  fail "Developer Guide must not describe assignees as the active-work lock"
fi
grep -Fq 'active or unresolved claim' docs/milestones/MILESTONE-003.md || fail "M003 must route an existing branch through claim resolution"
grep -Fq 'exact remote `issue/923` branch' docs/milestones/MILESTONE-003.md || fail "M003 must use the deterministic unassigned-work branch"
if grep -Fq 'sole assignee' docs/milestones/MILESTONE-003.md; then
  fail "M003 must not use sole assignee state as the work lock"
fi
if grep -Eiq '#686.*(assigned|assignee)|Keep the current assignee|spike; assignee' docs/milestones/MILESTONE-003.md; then
  fail "M003 must not describe #686 as assigned"
fi
grep -Fq 'bounded remaining shell-support decision or evidence task' docs/milestones/MILESTONE-003.md || fail "M003 must refine #686 against the accepted shell-integration decision"
for policy_file in AGENTS.md docs/engineering/DEVELOPMENT.md docs/engineering/ISSUE-PROTOCOL.md "$claim_skill" "$refine_skill"; do
  for forbidden in 'gh issue edit' '--add-assignee' '--remove-assignee' 'assignees.add' 'assignees.remove'; do
    if grep -Fq -- "$forbidden" "$policy_file"; then
      fail "claim policy must not mutate assignees: ${policy_file} contains ${forbidden}"
    fi
  done
done

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
