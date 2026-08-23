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
grep -q 'ANTHROPIC_SKILLS_DIR=' scripts/bootstrap-dev.sh || fail "Anthropic skill checkout is not managed locally"
grep -q 'checkout --detach "${FRONTEND_DESIGN_REF}"' scripts/bootstrap-dev.sh || fail "frontend-design source is not checked out at the pinned commit"
grep -q '"${ANTHROPIC_SKILLS_DIR}/skills/frontend-design"' scripts/bootstrap-dev.sh || fail "frontend-design is not installed from the verified local checkout"
if grep -q 'github.com/anthropics/skills/tree/' scripts/bootstrap-dev.sh; then
  fail "frontend-design bootstrap must not encode a commit SHA as a GitHub tree/branch URL"
fi

missing="$(mktemp)"
trap 'rm -f "$missing"' EXIT
if SEYAL_RUSTUP="${ROOT}/.definitely-missing-rustup" bash scripts/check-toolchain.sh >"$missing" 2>&1; then
  fail "missing rustup condition unexpectedly succeeded"
fi
grep -q 'rustup is required' "$missing" || fail "missing rustup failure is not actionable"

printf '[seyal tooling test] deterministic task/toolchain metadata tests passed.\n'
