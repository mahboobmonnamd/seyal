# M001 Composer History Fuzzy Search

**Status:** Frozen UI reference specification  
**Parent:** `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`  
**Scope:** Pane-scoped composer history discovery

## 1. Purpose

History fuzzy search lets power users recall and reuse commands without leaving the focused pane or opening a separate history page.

It is a composer capability, not an always-visible panel.

## 2. Invocation

History search opens contextually above the focused pane's composer through an intentional trigger, for example:

- keyboard history shortcut;
- composer history affordance;
- command-palette action.

The exact key binding is configurable/documented by the owning input spec.

## 3. Scope

Search is bound to the focused pane/composer context but may query broader command history according to product policy.

Useful scopes may include:

- recent commands;
- frequent commands;
- current workspace;
- all retained command history.

The UI must clearly communicate scope when it changes results.

## 4. Result row

A result may show:

- command text;
- relative/absolute execution time;
- workspace/path context when useful;
- success/failure marker where reliably known.

Do not render fake metadata or infer execution success from output text.

## 5. Interaction

Keyboard-first behavior is mandatory:

- type to filter;
- up/down to navigate;
- Enter selects/inserts or runs according to explicit composer behavior;
- Escape dismisses;
- mouse selection remains supported.

Prefer **insert into composer for editing** as the safe default when command content may need review. A separate explicit run action may execute immediately where product policy allows.

## 6. Fuzzy matching

Matching should prioritize practical command recall rather than generic document search.

Possible ranking inputs:

- text fuzzy match;
- recency;
- frequency;
- workspace/path relevance;
- exact prefix/token match.

Ranking must remain fast on large retained histories and must not synchronously scan unbounded history on the UI/render/terminal hot path.

## 7. Agents and actions

The composer may expose sibling modes such as:

- History;
- Agents;
- Actions.

These modes share the same anchored helper surface but have separate data sources and semantics.

Do not mix shell-history rows and agent/action results into an ambiguous list without clear categorization.

## 8. Security/privacy

History may contain sensitive commands.

Requirements:

- respect history retention/privacy settings;
- do not leak command history across user/security boundaries;
- avoid sending history to cloud/agents merely to rank local fuzzy matches;
- redact or exclude secrets according to the future history/privacy policy where feasible.

## 9. Pane-scoped draft preservation

Opening/dismissing history search must preserve the pane's current composer draft.

If the user selects a history row for insertion, that row replaces/inserts into the focused pane composer only.

It must never populate another pane's composer.

## 10. Performance

History indexing/search is not allowed to block:

- PTY reads/writes;
- VT mutation;
- damage publication;
- renderer frame preparation;
- input delivery to a running TUI.

Use asynchronous/indexed retrieval appropriate to the eventual history architecture.

## 11. Functional-only rule

Tabs such as Recent/Frequent, counts, timestamps and scopes should appear only when the underlying history model can support them correctly.
