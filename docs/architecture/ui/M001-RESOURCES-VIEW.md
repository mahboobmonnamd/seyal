# M001 Resources View

**Status:** Frozen UI reference specification  
**Parent:** `M001-CORE-TERMINAL-REFERENCE-SCREEN.md`  
**Scope:** Resource inventory and inspection only

## 1. Purpose

The Resources view provides a compact operational inventory of infrastructure/runtime resources Seyal can actually observe through supported data sources.

It is not a decorative dashboard and must not imply integrations that do not exist.

## 2. Primary user goals

Users should be able to:

- inspect local and supported remote hosts;
- inspect real host/process/resource metrics;
- inspect Docker/Kubernetes resources only when corresponding integrations exist;
- jump from a resource to an appropriate terminal/workflow;
- understand resource health/attention without opening multiple shells.

## 3. Information architecture

The left resource navigation may group real sources such as:

- Local Machine;
- Remote Hosts;
- Docker;
- Kubernetes;
- Runtime/toolchain inventory;
- Cloud providers only when explicitly integrated;
- Saved Views based on real queries.

Do not render empty categories merely for product appearance.

## 4. Center content

The center surface depends on selected resource scope.

Possible backed-by-real-data modules:

### Hosts
- host name/identity;
- platform;
- connectivity/health state;
- CPU;
- memory;
- disk/load where measurable.

### Containers
- name/id;
- image;
- status;
- CPU/memory/network where available;
- uptime.

### Kubernetes
- context/cluster;
- namespace;
- node/pod status;
- allocated CPU/memory where supported.

### Processes
- PID;
- process name;
- user;
- CPU;
- memory;
- started/duration where available.

## 5. Inspector

Selecting a resource changes the inspector to that exact resource.

For a host, useful sections include:

- host identity;
- OS/platform;
- uptime;
- address/location where safe and meaningful;
- Seyal runtime/agent connectivity where applicable;
- system overview;
- top processes;
- available actions.

Actions may include only real operations, such as:

- open terminal on host;
- SSH/connect where configured;
- view logs where integration exists;
- run documented diagnostics;
- configure alerts where implemented.

## 6. Data-source authority

Resource data must come from explicit OS/runtime/integration APIs.

Do not scrape arbitrary terminal output to manufacture infrastructure state.

For local host/process metrics, prefer platform/runtime APIs. For Docker/Kubernetes/cloud, use explicit adapters/integrations with typed data contracts.

## 7. No hidden shell dependency

The Resources view itself is a non-terminal management surface.

Opening it must **not** start a shell, consume a PTY, mark a shell busy, or require a terminal composer.

If the visual mockup suggests a `shell busy with resource monitor` message, that is **not authoritative**. The spec wins: resource observation must remain independent from terminal execution unless the user explicitly launches a command in a terminal pane.

## 8. Refresh behavior

Refresh/auto-refresh must be bounded and appropriate to the source.

Rules:

- no busy polling;
- no per-resource thread by default;
- adaptive/low-frequency refresh for expensive sources;
- UI refresh must not compete with terminal hot-path work;
- stale/error state must be visible rather than silently displaying old data as current.

## 9. Saved views

Saved views, when implemented, are query/filter presets over real resource data. They do not own or duplicate resource authority.

Examples may include:

- High CPU;
- Memory Pressure;
- CrashLooping Pods.

Do not ship a saved-view category until the underlying query is supported.

## 10. Functional-only rule

Every chart, metric, status, action and category must have a real source and behavior. Omit unavailable data rather than fill visual space with estimates or placeholders.
