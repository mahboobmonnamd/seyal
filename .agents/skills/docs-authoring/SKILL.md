---
name: docs-authoring
description: Create or update Seyal user/developer documentation without duplicating architectural authority or claiming unfinished behavior.
---

# Documentation authoring

Use when a change adds or alters user-visible behavior, configuration, workflows, troubleshooting, contributor workflow, architecture orientation, screenshots, diagrams, or documentation media.

1. Read `site/README.md`, `AGENTS.md`, the implementation Issue, and every linked architecture/spec/milestone document.
2. Classify the audience:
   - **User Guide**: observable released behavior, tasks, configuration, troubleshooting, examples.
   - **Developer Guide**: contributor orientation, development workflow, architecture map, testing/performance/security guidance.
   - authoritative architecture/spec/ADR content stays under existing repository `docs/` authority paths.
3. Never document planned behavior as shipped. Use explicit labels such as `under development` when the implementation has not passed its milestone gates.
4. Prefer task-first titles and examples. Explain concepts only where they help the reader complete or understand a real workflow.
5. For developer architecture pages, summarize for orientation and link readers to the authoritative ADR/spec rather than copying decision text into a second source of truth.
6. Images are welcome when they add information. Use real current screenshots or maintained diagrams; add meaningful alt text.
7. Before adding screenshots, remove/redact tokens, credentials, customer data, private hostnames/paths, email addresses, or other sensitive data.
8. Do not use generated concept UI as evidence that the product works. Concept images must be labelled clearly when used for design discussion outside procedural user docs.
9. Add tutorial/generated video only after the documented UI/flow is stable. Record the demonstrated release/version/commit and provide a text alternative for essential instructions.
10. Update navigation when adding a durable documentation area. Avoid empty future-feature pages.
11. Finish by running the `docs-validation` skill and include documentation verification in the PR.

Documentation changes must remain completely outside the production terminal hot path and must not create an OSS dependency on commercial code or content.
