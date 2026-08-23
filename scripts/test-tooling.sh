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
grep -q 'github-mcp-server' scripts/bootstrap-dev.sh || fail "GitHub MCP bootstrap is missing"
grep -q 'mcpbridge' scripts/bootstrap-dev.sh || fail "official Xcode MCP bootstrap is missing"
grep -q 'xcodebuildmcp@${XCODEBUILD_MCP_VERSION}' scripts/bootstrap-dev.sh || fail "XcodeBuildMCP configuration is missing"
grep -q '^configure_copilot()' scripts/bootstrap-dev.sh || fail "GitHub Copilot MCP setup is missing"
grep -q 'configure_mcp_client copilot "GitHub Copilot CLI" builtin' scripts/bootstrap-dev.sh || fail "Copilot must use built-in GitHub MCP mode"
grep -q 'if has claude || has codex; then' scripts/bootstrap-dev.sh || fail "external GitHub MCP should only be provisioned for clients that need it"

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
