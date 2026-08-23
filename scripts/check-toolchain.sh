#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLCHAIN_FILE="${ROOT}/rust-toolchain.toml"
RUSTUP="${SEYAL_RUSTUP:-rustup}"

fail() {
  printf '[seyal toolchain] ERROR: %s\n' "$*" >&2
  exit 1
}

has_command() {
  local candidate="$1"
  if [[ "$candidate" == */* ]]; then
    [[ -x "$candidate" ]]
  else
    command -v "$candidate" >/dev/null 2>&1
  fi
}

pinned_channel() {
  sed -nE 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "$TOOLCHAIN_FILE" | head -n1
}

[[ -f "$TOOLCHAIN_FILE" ]] || fail "missing rust-toolchain.toml"
channel="$(pinned_channel)"
[[ -n "$channel" ]] || fail "rust-toolchain.toml does not declare a channel"
grep -Eq 'components[[:space:]]*=[[:space:]]*\[[^]]*"rustfmt"' "$TOOLCHAIN_FILE" || fail "pinned toolchain must include rustfmt"
grep -Eq 'components[[:space:]]*=[[:space:]]*\[[^]]*"clippy"' "$TOOLCHAIN_FILE" || fail "pinned toolchain must include clippy"

has_command "$RUSTUP" || fail "rustup is required. Install rustup from https://rustup.rs/ before running make bootstrap."

if ! "$RUSTUP" toolchain list | grep -Eq "^${channel}(-|[[:space:]])"; then
  fail "Rust toolchain ${channel} is not installed. Run 'make bootstrap'."
fi

"$RUSTUP" run "$channel" rustc --version >/dev/null || fail "rustc is unavailable for ${channel}"
"$RUSTUP" run "$channel" cargo --version >/dev/null || fail "cargo is unavailable for ${channel}"
"$RUSTUP" run "$channel" rustfmt --version >/dev/null || fail "rustfmt is unavailable for ${channel}"
"$RUSTUP" run "$channel" cargo clippy --version >/dev/null || fail "clippy is unavailable for ${channel}"

actual="$($RUSTUP run "$channel" rustc --version | awk '{print $2}')"
[[ "$actual" == "$channel" ]] || fail "expected rustc ${channel}, found ${actual}"

printf '[seyal toolchain] Rust %s with cargo, rustfmt and clippy is ready.\n' "$channel"
