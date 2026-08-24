---
name: docs-validation
description: Validate Seyal documentation for correctness, source-of-truth alignment, build health, links, accessibility, security, and stale media.
---

# Documentation validation

Use before merging documentation or any change whose documentation impact is non-empty.

1. Read `site/README.md`, the related Issue/PR, and the authoritative architecture/spec/milestone documents for the behavior being described.
2. Verify audience placement: user tasks in User Guide; contributor orientation in Developer Guide; decisions/specifications remain in authoritative repository docs.
3. Reject claims that an unimplemented or unvalidated feature is available. Check commands, paths, option names, shortcuts, UI labels, defaults, and limitations against current code/spec evidence.
4. Check links and navigation. No orphaned durable pages, broken internal links, or duplicate competing source-of-truth pages.
5. From `site/`, install dependencies using the repository-supported package workflow and run at minimum:

```sh
npm run build
npm run check
```

6. Verify images render, have useful alt text, and contain no secrets, credentials, customer data, tokens, private infrastructure details, or irrelevant personal information.
7. For screenshots/video, verify the demonstrated UI still matches the current implementation and that the media identifies the relevant release/version/commit when needed.
8. Check accessibility basics: heading order, descriptive links, text alternatives, keyboard-usable interactive components, and no instruction that depends only on color/visual position.
9. Check examples for copy/paste safety. Never publish destructive commands without explicit warning/context.
10. Confirm docs tooling, analytics, search, media, and commercial publishing introduce no dependency into terminal runtime hot paths.
11. Report validation evidence in the PR. If the site cannot build from a clean checkout, documentation is not Done.
