# Tooling scope

Seyal development tooling is intentionally limited to capabilities required to build, test, debug and govern the native terminal product.

The agent bootstrap must not install general web-design, browser-automation, third-party documentation-indexing or unrelated developer tooling merely because it may be useful in other projects.

Approved external developer integrations are currently limited to:

- GitHub MCP for repository, issue, pull-request and CI workflow;
- Apple's official Xcode MCP bridge when the installed Xcode provides it;
- XcodeBuildMCP for native macOS build, test, run, screenshot, UI hierarchy and debugging workflows.

Seyal-owned project skills remain versioned in `.agents/skills/` with tool-specific adapters where required.

A new external skill, MCP server or developer dependency may be added only for a concrete Seyal need that the existing native toolchain cannot satisfy. The Issue/PR must explain that need, reproducibility, permissions/security impact and why the new tool belongs in this project.
