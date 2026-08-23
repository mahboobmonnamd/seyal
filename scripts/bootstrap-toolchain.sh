#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUSTUP="${SEYAL_RUSTUP:-rustup}"

info() { printf '[seyal bootstrap] %s\n' "$*"; }
fail() { printf '[seyal bootstrap] ERROR: %s\n' "$*" >&2; exit 1; }
has() { command -v "$1" >/dev/null 2>&1; }

validate_host_prerequisites() {
  has git || fail "git is required before bootstrap"
  has make || fail "make is required before bootstrap"

  if [[ "$(uname -s)" == "Darwin" ]]; then
    has xcode-select || fail "Xcode is required. Install it from Apple before bootstrap."
    if ! xcode-select -p >/dev/null 2>&1; then
      fail "no active Xcode developer directory; install/select Xcode before bootstrap"
    fi
    has xcrun || fail "xcrun is required from Xcode"
    xcrun --find clang >/dev/null 2>&1 || fail "clang is unavailable from the active Xcode installation"
    bash "${ROOT}/scripts/check-macos-toolchain.sh"
  fi
}

install_pinned_rust_toolchain() {
  if ! command -v "$RUSTUP" >/dev/null 2>&1 && [[ "$RUSTUP" != */* ]]; then
    fail "rustup is required. Install rustup from https://rustup.rs/ before running make bootstrap."
  fi
  if [[ "$RUSTUP" == */* && ! -x "$RUSTUP" ]]; then
    fail "rustup is required. Install rustup from https://rustup.rs/ before running make bootstrap."
  fi

  local channel
  channel="$(sed -nE 's/^[[:space:]]*channel[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "${ROOT}/rust-toolchain.toml" | head -n1)"
  [[ -n "$channel" ]] || fail "rust-toolchain.toml does not declare a channel"

  info "installing/verifying pinned Rust ${channel} (minimal + rustfmt + clippy)"
  "$RUSTUP" toolchain install "$channel" --profile minimal --component rustfmt --component clippy
}

initialize_submodules() {
  if [[ -f "${ROOT}/.gitmodules" ]]; then
    info "initializing pinned git submodules"
    git -C "$ROOT" submodule update --init --recursive
  fi
}

main() {
  validate_host_prerequisites
  [[ -f "${ROOT}/rust-toolchain.toml" ]] || fail "missing rust-toolchain.toml"
  install_pinned_rust_toolchain
  initialize_submodules
  bash "${ROOT}/scripts/check-toolchain.sh"
  info "complete"
  info "optional agent/MCP provisioning is separate: make bootstrap-agents"
}

main "$@"
