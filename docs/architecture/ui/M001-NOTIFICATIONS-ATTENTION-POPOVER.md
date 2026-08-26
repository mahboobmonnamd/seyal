# M001 Notifications and Attention Popover

**Status:** Frozen UI reference specification  
**Parent:** `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`  
**Scope:** Global attention indicator and compact action popover

## 1. Purpose

The notification/attention system lets users handle important background events without navigating away from their current terminal context.

The bell in the top-right is the stable entry point.

## 2. What belongs here

Only events that merit user awareness or action should enter this surface.

Examples:

- agent approval required;
- command/process failed;
- long-running/background task completed;
- execution needs attention;
- reconnect/session problem;
- explicit product/system warning.

Do not turn ordinary terminal output into notifications.

## 3. Attention item model

Every rendered item must map to one authoritative attention/event object with:

- stable identity;
- source type/id;
- workspace/tab/pane association where applicable;
- severity/state;
- created/updated time;
- action requirements;
- resolved/read state.

The popover, Agents view and future attention center must reference the same underlying object rather than maintain duplicate copies.

## 4. Popover layout

The bell opens a compact stack anchored below/top-right of the app chrome.

Each item may contain:

- source/status icon;
- concise title;
- one- or two-line context;
- relative time;
- only the actions needed for that item.

Examples:

### Agent approval
- Approve;
- Reject;
- jump/open details when needed.

### Command complete
- View output.

### Process failed
- Open logs / jump to Block.

Do not add buttons merely for symmetry.

## 5. Approval actions

Approval/rejection from the popover is intentionally supported so users do not need to switch tabs.

Rules:

- action must resolve against the authoritative agent/approval state;
- stale/already-resolved approvals fail safely and refresh UI;
- approval action must not directly mutate terminal state;
- security-sensitive details should be shown before approval when required by policy.

## 6. Navigation

Selecting an informational item should jump to the owning workspace/tab/pane/Block/agent when that target still exists.

If the target no longer exists, show the retained event/details rather than silently creating a new execution.

## 7. Read vs resolved

`Read` and `Resolved` are different concepts.

- **Read**: user has seen the item.
- **Resolved**: the underlying action/condition no longer needs attention.

Mark-all-read must never approve/reject/resolve actionable work.

## 8. Noise control

The system should coalesce repetitive events from the same source where possible.

Examples:

- one long-running process finishing should produce one completion event;
- repeated equivalent agent prompts should not create uncontrolled duplicate cards;
- high-volume terminal logs must never emit one notification per line/error token.

## 9. Performance

Attention production/aggregation is asynchronous and bounded.

Terminal I/O/rendering must never wait for:

- notification persistence;
- badge updates;
- popover rendering;
- agent approval UI.

## 10. Privacy and security

Notification previews should minimize raw terminal/secrets exposure.

Do not place sensitive command output, environment variables, tokens or credentials in preview text by default.

## 11. Functional-only rule

The unread badge, timestamps, actions and status colors must reflect actual state. No fake unread counts or decorative alerts are permitted.
