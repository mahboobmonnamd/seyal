#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${HOME}/.local/bin"
WRAPPER="${BIN_DIR}/seyal-github-mcp"

info() { printf '[seyal agent bootstrap] %s\n' "$*"; }
warn() { printf '[seyal agent bootstrap] WARN: %s\n' "$*" >&2; }
has() { command -v "$1" >/dev/null 2>&1; }

install_github_wrapper() {
  has github-mcp-server || {
    warn "github-mcp-server is not installed; install the official server explicitly before configuring GitHub MCP"
    return 1
  }

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

configure_client() {
  local client="$1"
  has "$client" || { warn "${client} CLI not found; skipping its MCP setup"; return 0; }

  if has xcrun && xcrun --find mcpbridge >/dev/null 2>&1; then
    if ! mcp_present "$client" xcode; then
      info "configuring Xcode MCP for ${client}"
      "$client" mcp add xcode -- xcrun mcpbridge
    fi
  else
    warn "xcrun mcpbridge unavailable; Xcode MCP not configured for ${client}"
  fi

  if [[ -x "${WRAPPER}" ]] && ! mcp_present "$client" github; then
    info "configuring GitHub MCP for ${client}"
    "$client" mcp add github -- "${WRAPPER}"
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
  install_github_wrapper || true
  configure_client claude
  configure_client codex
  info "complete; no credentials or optional packages were installed"
}

main "$@"
