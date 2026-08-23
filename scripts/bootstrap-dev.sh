#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${HOME}/.local/bin"
STATE_DIR="${HOME}/.local/state/seyal/bootstrap"
DATA_DIR="${HOME}/.local/share/seyal"
GITHUB_WRAPPER="${BIN_DIR}/seyal-github-mcp"
ANTHROPIC_SKILLS_DIR="${DATA_DIR}/skills/anthropic-skills"
APPLE_DEEP_DOCS_WRAPPER="${BIN_DIR}/seyal-apple-deep-docs-mcp"
APPLE_DEEP_DOCS_DIR="${DATA_DIR}/mcp/appledeepdoc-mcp"
APPLE_DEEP_DOCS_VENV="${DATA_DIR}/venv/appledeepdoc-mcp"

# Reviewed/pinned developer-tool inputs. Update only through a normal Seyal PR.
FRONTEND_DESIGN_REF="3b3fad96af16a10759d930941b4520ba0c40edae"
XCODEBUILD_MCP_VERSION="2.7.0"
PLAYWRIGHT_MCP_VERSION="0.0.79"
APPLE_DEEP_DOCS_REF="5087bd04fb0cf6cb5dda422dcda798506a678df4"

info() { printf '[seyal bootstrap] %s\n' "$*"; }
warn() { printf '[seyal bootstrap] WARN: %s\n' "$*" >&2; }
has() { command -v "$1" >/dev/null 2>&1; }

ensure_submodules() {
  if [[ -f "${ROOT}/.gitmodules" ]]; then
    info "initializing pinned git submodules"
    git -C "${ROOT}" submodule update --init --recursive
  fi
}

prepare_frontend_design_source() {
  if ! has git; then
    warn "git not found; cannot prepare pinned Anthropic frontend-design source"
    return 1
  fi

  mkdir -p "$(dirname "${ANTHROPIC_SKILLS_DIR}")"

  if [[ -d "${ANTHROPIC_SKILLS_DIR}/.git" ]]; then
    if [[ -n "$(git -C "${ANTHROPIC_SKILLS_DIR}" status --porcelain)" ]]; then
      warn "managed Anthropic skills checkout is dirty; refusing to overwrite it"
      return 1
    fi
    git -C "${ANTHROPIC_SKILLS_DIR}" fetch --prune origin
  else
    rm -rf "${ANTHROPIC_SKILLS_DIR}"
    info "cloning Anthropic skills source for pinned frontend-design"
    git clone --filter=blob:none https://github.com/anthropics/skills.git "${ANTHROPIC_SKILLS_DIR}"
  fi

  if ! git -C "${ANTHROPIC_SKILLS_DIR}" cat-file -e "${FRONTEND_DESIGN_REF}^{commit}" 2>/dev/null; then
    git -C "${ANTHROPIC_SKILLS_DIR}" fetch origin "${FRONTEND_DESIGN_REF}"
  fi

  git -C "${ANTHROPIC_SKILLS_DIR}" checkout --detach "${FRONTEND_DESIGN_REF}"

  local actual_ref
  actual_ref="$(git -C "${ANTHROPIC_SKILLS_DIR}" rev-parse HEAD)"
  if [[ "${actual_ref}" != "${FRONTEND_DESIGN_REF}" ]]; then
    warn "Anthropic skills checkout mismatch: expected ${FRONTEND_DESIGN_REF}, found ${actual_ref}"
    return 1
  fi

  if [[ ! -f "${ANTHROPIC_SKILLS_DIR}/skills/frontend-design/SKILL.md" ]]; then
    warn "pinned Anthropic checkout does not contain skills/frontend-design/SKILL.md"
    return 1
  fi
}

ensure_frontend_design_skill() {
  if ! has npx; then
    warn "npx not found; pinned Anthropic frontend-design skill not installed"
    return 0
  fi

  local agent_args=()
  local needs_install=0

  if has claude; then
    agent_args+=(--agent claude-code)
    [[ -f "${HOME}/.claude/skills/frontend-design/SKILL.md" ]] || needs_install=1
  fi
  if has codex; then
    agent_args+=(--agent codex)
    [[ -f "${HOME}/.codex/skills/frontend-design/SKILL.md" ]] || needs_install=1
  fi
  if has copilot; then
    agent_args+=(--agent github-copilot)
    [[ -f "${HOME}/.copilot/skills/frontend-design/SKILL.md" ]] || needs_install=1
  fi

  if (( ${#agent_args[@]} == 0 )); then
    warn "no supported coding-agent CLI found; skipping external frontend-design skill"
    return 0
  fi

  mkdir -p "${STATE_DIR}"
  local marker="${STATE_DIR}/frontend-design.ref"
  local installed_ref=""
  [[ -f "${marker}" ]] && installed_ref="$(cat "${marker}")"

  if [[ "${installed_ref}" == "${FRONTEND_DESIGN_REF}" && "${needs_install}" -eq 0 ]]; then
    return 0
  fi

  prepare_frontend_design_source

  info "installing pinned Anthropic frontend-design skill for web prototypes"
  DISABLE_TELEMETRY=1 npx -y skills add \
    "${ANTHROPIC_SKILLS_DIR}/skills/frontend-design" \
    --global "${agent_args[@]}" --copy --yes
  printf '%s\n' "${FRONTEND_DESIGN_REF}" >"${marker}"
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
  cat >"${GITHUB_WRAPPER}" <<'EOF'
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
  chmod 0755 "${GITHUB_WRAPPER}"
}

ensure_apple_deep_docs() {
  [[ "${SEYAL_ENABLE_APPLE_DEEP_DOCS:-0}" == "1" ]] || return 0

  if ! has git || ! has python3; then
    warn "AppleDeepDocs opt-in requires git and Python 3; skipping"
    return 0
  fi

  mkdir -p "$(dirname "${APPLE_DEEP_DOCS_DIR}")" "$(dirname "${APPLE_DEEP_DOCS_VENV}")" "${BIN_DIR}"

  if [[ -d "${APPLE_DEEP_DOCS_DIR}/.git" ]]; then
    if [[ -n "$(git -C "${APPLE_DEEP_DOCS_DIR}" status --porcelain)" ]]; then
      warn "managed AppleDeepDocs checkout is dirty; refusing to overwrite it"
      return 0
    fi
    git -C "${APPLE_DEEP_DOCS_DIR}" fetch --depth 1 origin "${APPLE_DEEP_DOCS_REF}"
  else
    rm -rf "${APPLE_DEEP_DOCS_DIR}"
    git clone --filter=blob:none https://github.com/Ahrentlov/appledeepdoc-mcp.git "${APPLE_DEEP_DOCS_DIR}"
  fi

  git -C "${APPLE_DEEP_DOCS_DIR}" checkout --detach "${APPLE_DEEP_DOCS_REF}"

  if [[ ! -x "${APPLE_DEEP_DOCS_VENV}/bin/python" ]]; then
    python3 -m venv "${APPLE_DEEP_DOCS_VENV}"
  fi
  "${APPLE_DEEP_DOCS_VENV}/bin/python" -m pip install --disable-pip-version-check -e "${APPLE_DEEP_DOCS_DIR}"

  cat >"${APPLE_DEEP_DOCS_WRAPPER}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
export CODE_EXECUTION_MODE=true
cd "${APPLE_DEEP_DOCS_DIR}"
exec "${APPLE_DEEP_DOCS_VENV}/bin/python" main.py
EOF
  chmod 0755 "${APPLE_DEEP_DOCS_WRAPPER}"
}

mcp_present() {
  local client="$1" name="$2"
  "$client" mcp list 2>/dev/null | grep -Eq "(^|[[:space:]])${name}([[:space:]:]|$)"
}

configure_mcp_client() {
  local client="$1" label="$2"

  if has xcrun && xcrun --find mcpbridge >/dev/null 2>&1; then
    if ! mcp_present "${client}" xcode; then
      info "configuring official Xcode MCP for ${label}"
      "${client}" mcp add xcode -- xcrun mcpbridge
    fi
  else
    warn "xcrun mcpbridge unavailable; Xcode MCP not configured for ${label}"
  fi

  if [[ -x "${GITHUB_WRAPPER}" ]] && ! mcp_present "${client}" github; then
    info "configuring GitHub MCP for ${label}"
    "${client}" mcp add github -- "${GITHUB_WRAPPER}"
  fi

  if has npx; then
    if ! mcp_present "${client}" xcodebuild; then
      info "configuring XcodeBuildMCP ${XCODEBUILD_MCP_VERSION} for ${label}"
      "${client}" mcp add xcodebuild -- npx -y "xcodebuildmcp@${XCODEBUILD_MCP_VERSION}" mcp
    fi

    if ! mcp_present "${client}" playwright; then
      info "configuring Playwright MCP ${PLAYWRIGHT_MCP_VERSION} for ${label} (web prototypes only)"
      "${client}" mcp add playwright -- npx -y "@playwright/mcp@${PLAYWRIGHT_MCP_VERSION}"
    fi
  else
    warn "npx not found; XcodeBuildMCP/Playwright MCP not configured for ${label}"
  fi

  if [[ -x "${APPLE_DEEP_DOCS_WRAPPER}" ]] && ! mcp_present "${client}" apple-deep-docs; then
    info "configuring opt-in AppleDeepDocs MCP for ${label}"
    "${client}" mcp add apple-deep-docs -- "${APPLE_DEEP_DOCS_WRAPPER}"
  fi
}

configure_claude() {
  has claude || { warn "Claude Code CLI not found; skipping Claude MCP setup"; return 0; }
  configure_mcp_client claude "Claude Code"
}

configure_codex() {
  has codex || { warn "Codex CLI not found; skipping Codex MCP setup"; return 0; }
  configure_mcp_client codex "Codex"
}

verify_repo_skills() {
  local required=(
    architecture-change implement-issue issue-refinement milestone-validation
    performance-gate pr-review security-review vt-tdd
    macos-native-design macos-ui-testing macos-accessibility visual-regression
    terminal-conformance metal-renderer rust-fuzzing apple-platform-docs image-to-code
  )
  local skill
  for skill in "${required[@]}"; do
    [[ -f "${ROOT}/.agents/skills/${skill}/SKILL.md" ]] || {
      echo "missing canonical skill: ${skill}" >&2
      exit 1
    }
    [[ -f "${ROOT}/.claude/skills/${skill}/SKILL.md" ]] || {
      echo "missing Claude adapter: ${skill}" >&2
      exit 1
    }
  done
}

main() {
  verify_repo_skills
  ensure_submodules
  ensure_frontend_design_skill

  if ensure_github_mcp_binary; then
    install_github_wrapper
  fi

  ensure_apple_deep_docs
  configure_claude
  configure_codex

  info "complete"
  info "Seyal skills are versioned in .agents/skills; external developer tools are pinned; no credentials were written"
  if [[ "${SEYAL_ENABLE_APPLE_DEEP_DOCS:-0}" != "1" ]]; then
    info "AppleDeepDocs remains opt-in: SEYAL_ENABLE_APPLE_DEEP_DOCS=1 make bootstrap"
  fi
}

main "$@"
