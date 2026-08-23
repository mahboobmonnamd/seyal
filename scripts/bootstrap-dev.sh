#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${HOME}/.local/bin"
WRAPPER="${BIN_DIR}/seyal-github-mcp"

info() { printf '[seyal bootstrap] %s\n' "$*"; }
warn() { printf '[seyal bootstrap] WARN: %s\n' "$*" >&2; }
has() { command -v "$1" >/dev/null 2>&1; }

ensure_submodules() {
  if [[ -f "${ROOT}/.gitmodules" ]]; then
    info "initializing pinned git submodules"
    git -C "${ROOT}" submodule update --init --recursive
  fi
}

ensure_github_mcp_binary() {
  if has github-mcp-server; then
    return 0
  fi
  if [[ "$(uname -s)" == "Darwin" ]] && has brew; then
    info "installing official GitHub MCP server with Homebrew"
    brew install github-mcp-server
  else
    warn "github-mcp-server not found; install it manually to enable GitHub MCP"
    return 1
  fi
}

install_github_wrapper() {
  mkdir -p "${BIN_DIR}"
  cat >"${WRAPPER}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if ! command -v github-mcp-server >/dev/null 2>&1; then
  echo "github-mcp-server is not installed" >&2
  exit 127
fi

if [[ -n "${GITHUB_PERSONAL_ACCESS_TOKEN:-}" ]]; then
  exec github-mcp-server stdio
fi

if [[ -n "${GITHUB_PAT_TOKEN:-}" ]]; then
  export GITHUB_PERSONAL_ACCESS_TOKEN="${GITHUB_PAT_TOKEN}"
  exec github-mcp-server stdio
fi

if command -v gh >/dev/null 2>&1; then
  token="$(gh auth token 2>/dev/null || true)"
  if [[ -n "${token}" ]]; then
    export GITHUB_PERSONAL_ACCESS_TOKEN="${token}"
    exec github-mcp-server stdio
  fi
fi

echo "GitHub MCP needs authentication. Set GITHUB_PAT_TOKEN or authenticate with 'gh auth login'." >&2
exit 78
EOF
  chmod 0755 "${WRAPPER}"
}

mcp_present() {
  local client="$1" name="$2"
  "$client" mcp list 2>/dev/null | grep -Eq "(^|[[:space:]])${name}([[:space:]:]|$)"
}

configure_claude() {
  has claude || { warn "Claude Code CLI not found; skipping Claude MCP setup"; return 0; }

  if has xcrun && xcrun --find mcpbridge >/dev/null 2>&1; then
    if ! mcp_present claude xcode; then
      info "configuring Xcode MCP for Claude Code"
      claude mcp add xcode -- xcrun mcpbridge
    fi
  else
    warn "xcrun mcpbridge unavailable; Xcode MCP not configured for Claude"
  fi

  if [[ -x "${WRAPPER}" ]] && ! mcp_present claude github; then
    info "configuring GitHub MCP for Claude Code"
    claude mcp add github -- "${WRAPPER}"
  fi
}

configure_codex() {
  has codex || { warn "Codex CLI not found; skipping Codex MCP setup"; return 0; }

  if has xcrun && xcrun --find mcpbridge >/dev/null 2>&1; then
    if ! mcp_present codex xcode; then
      info "configuring Xcode MCP for Codex"
      codex mcp add xcode -- xcrun mcpbridge
    fi
  else
    warn "xcrun mcpbridge unavailable; Xcode MCP not configured for Codex"
  fi

  if [[ -x "${WRAPPER}" ]] && ! mcp_present codex github; then
    info "configuring GitHub MCP for Codex"
    codex mcp add github -- "${WRAPPER}"
  fi
}

verify_repo_skills() {
  local required=(
    architecture-change implement-issue issue-refinement milestone-validation
    performance-gate pr-review security-review vt-tdd
    macos-native-design macos-ui-testing macos-accessibility visual-regression
  )
  local skill
  for skill in "${required[@]}"; do
    [[ -f "${ROOT}/.agents/skills/${skill}/SKILL.md" ]] || {
      echo "missing canonical skill: ${skill}" >&2
      exit 1
    }
  done
}

main() {
  verify_repo_skills
  ensure_submodules

  if ensure_github_mcp_binary; then
    install_github_wrapper
  fi

  configure_claude
  configure_codex

  info "complete"
  info "canonical Seyal skills are versioned in .agents/skills; no credentials were written"
}

main "$@"
