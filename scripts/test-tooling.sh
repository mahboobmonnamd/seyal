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
grep -q 'if has claude || has codex; then' scripts/bootstrap-dev.sh || fail "external GitHub MCP should only be provisioned for clients that need it"

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
