# M001 Cycle-Time and Execution Analysis

**Purpose:** Preserve M001 engineering rigor while reducing avoidable wall-clock/rework cost in future milestones.  
**Evidence window:** GitHub issue/PR/workflow history through 2026-08-27 15:24Z.  
**Important:** This is process analysis only. It does not change M001 acceptance criteria or authorize merging active work.

## Observed M001 pass evidence

| Work | GitHub evidence | Approx. elapsed | Change shape | Workflow-run amplification |
|---|---|---:|---|---:|
| Pass 3 — PTY/child lifecycle, #28 / PR #40 | issue 2026-08-23 17:28Z -> 2026-08-24 04:05Z; PR 17:56Z -> 04:05Z | issue ~10.6 h; PR ~10.1 h | 25 commits, 27 files, +2,700/-601 | 18 branch workflow runs |
| Pass 4 architecture refinement, #70 / PR #72 | PR 04:36Z -> 04:49Z | ~13 min | 1 documentation commit | small |
| Pre-Pass-4 corrections, PRs #77/#79/#81/#85/#90/#93/#97 | 05:01Z -> 10:22Z across several independent packages | several overlapping hours | LineId, alt screen, runtime safety/continuity, terminfo, conformance and scalability evidence | multiple independent exact-head runs |
| Pass 4 implementation, #99 / PR #100 | issue 10:31Z -> 12:16Z; PR 10:39Z -> 12:16Z | issue ~1.75 h; PR ~1.6 h | 68 commits, 30 files, +3,509/-1,699 | 54 branch workflow runs |
| Pass 5 spec, #103 / PR #104 | PR 13:05Z -> 13:22Z | ~17 min | 1 documentation commit | small |
| Pass 5 implementation baseline, #105 / PR #106 | PR 2026-08-24 13:35Z -> 2026-08-25 09:22Z | ~19.8 h | 213 commits, 69 files, +14,153/-1,002 | 193 branch workflow runs |
| Pass 5.1 hardening, #651 / PR #652 | PR 2026-08-25 09:35Z -> 2026-08-27 01:37Z | ~40.0 h | 72 commits, 29 files, +7,032/-463 | 96 branch workflow runs |
| Pass 6 permanent Metal renderer, #658 / PR #659 | opened 2026-08-27; still open at evidence cutoff | active | implementation + corrections; head continued to move during review | **118** branch workflow runs at 15:24Z |

Workflow-run counts are an amplification indicator, not CI minutes. They include multiple workflow types and historical heads. They must not be interpreted as 118 full release validations.

## What the evidence says

### 1. “One pass is three days” is not the real unit

Some passes completed in hours, while Pass 5/5.1 stretched across roughly 2.5 calendar days. The variable is not the word *pass*; it is uncertainty, size of the change set, review discovery and number of heads forced through expensive evidence gates.

M001 therefore should not be forecast as `remaining passes × historical average`. Future planning should forecast by uncertainty class:
- known implementation behind accepted contract;
- architecture/R&D unresolved;
- platform/native integration;
- performance/security hardening;
- release/evidence integration.

### 2. Pass 5 was too large to be a comfortable review unit

PR #106 reached 213 commits, 69 files and more than 14k additions before the final review still found eight mandatory hardening areas: high-output performance, recovery amplification, p99 fanout methodology, protocol/reconnect fuzz, display-result churn, listener peer credentials/FD flags, ancillary FD validation and whole-Runtime resource auditing.

The lesson is **not** to remove the final adversarial review. The lesson is to make those categories explicit before implementation and to land independently provable permanent slices when architecture permits.

### 3. Pass 5.1 proves quality gates find real work

The hardening follow-up was substantial: 72 commits and ~7k additions. That is evidence that security/performance/failure review is productive, not ceremony. Any process optimization that skips it would simply convert development time into production defects.

### 4. CI amplification is a material source of waste

Pass 4 implementation recorded 54 branch workflow runs, Pass 5 baseline 193, Pass 5.1 96, and active Pass 6 reached 118. A single final Foundation Quality run can be only minutes; the larger cost comes from re-running broad suites across many intermediate heads.

The optimization target is therefore:
- fewer speculative heads;
- more local/targeted pre-push evidence;
- cancel superseded CI;
- cache safely;
- run affected fast gates continuously;
- reserve full exact-head Foundation/fuzz/native/bench evidence for meaningful review checkpoints and final acceptance.

This preserves the same final gate.

### 5. Review discovery is currently late

Several Pass 5 blockers were categories that can be demanded in a Ready checklist before code: adversarial reconnect state machine, peer credential/FD handling, benchmark percentile methodology, recovery amplification, resource ceilings and failure-injection coverage.

Late discovery causes architecture churn + code churn + repeated CI. Earlier review moves the same rigor left.

## Permanent process changes for M002+

### Ready Gate before implementation

Every implementation package must include, before coding:
1. authority/ownership statement;
2. dependency and state-machine boundaries;
3. failure and security matrix;
4. benchmark methodology and required percentile/resource outputs;
5. fuzz/property/conformance plan where applicable;
6. exact platform/runtime assumptions;
7. explicit non-goals;
8. independent adversarial Ready review.

A package with unresolved architecture goes to a spike, not directly to implementation.

### Smaller permanent slices, same milestone gate

Milestones remain the product gate. Within them, prefer reviewable slices that each leave master permanently correct. Examples for M002:
- compatibility/query/mode breadth;
- Unicode/width policy after #684;
- scrollback/reflow after #685;
- mouse/selection/link/search;
- exact performance/scaling gate.

Do not split a change when doing so would create two temporary terminal authorities or an invalid intermediate architecture.

### CI tiers without weaker evidence

**Tier A — local/pre-push:** format/lint/affected unit tests, deterministic fixtures and targeted benchmarks.

**Tier B — branch fast CI:** affected packages + compile/check + relevant contract tests. Cancel superseded runs using concurrency groups.

**Tier C — review checkpoint:** full Foundation Quality plus the package's required fuzz/security/native/perf suites.

**Tier D — exact-head acceptance:** complete milestone-required evidence on the final executable SHA.

A test is never removed merely because it is slow. Slow redundant executions are reduced by better scheduling, caching and fewer meaningless heads.

### Benchmark contract before code

Performance issues such as #673 must define:
- exact workload and dimensions;
- hardware/OS/build metadata;
- p50/p95/p99 methodology;
- latency vs throughput vs resource metrics;
- allowed regression budget;
- host/platform limits separated from Seyal limits.

This prevents “benchmark correction” from becoming a late pass of its own.

### Evidence manifest

For each review checkpoint, record an evidence manifest keyed to git SHA:
- test/fuzz/native/benchmark run identifiers;
- hardware metadata;
- known platform limitations;
- executable vs documentation-only head distinction.

Unchanged evidence may be referenced where technically valid, but final release acceptance still names the exact executable head.

### Two-milestone look-ahead

While M001 implementation continues, high-risk M002/M003/M004 decisions can be resolved in spikes. This is how future cycle time improves without weakening implementation quality. Production code remains milestone-gated.

### Review ownership

At three contributors:
- one person owns terminal/runtime critical-path changes;
- one owns native/workspace boundaries;
- one owns conformance/performance/release/spikes;
- a change's author cannot be the only architecture/security reviewer.

At larger team sizes, add subsystem reviewers before adding more simultaneous core branches.

## Recommended metrics to track

Track these per package and milestone:
- issue Ready -> first implementation PR;
- PR open -> first substantive review;
- first substantive review -> final executable head;
- final executable head -> merge approval;
- commits and changed files;
- number of executable heads that triggered Tier C/D CI;
- workflow runs and total CI minutes;
- review blockers found before code vs after code;
- benchmark/fuzz failures that found real defects;
- master-resync conflicts/rework.

The target is not “few commits” or “few tests”. The target is fewer **invalidated heads** and fewer **late-discovered requirement classes**.

## Decision

Keep M001-quality TDD, fuzzing, benchmarks, security review, independent review and exact-head CI. Reduce cycle time by resolving architecture earlier, shrinking review units where safe, making failure/performance criteria explicit before code, and preventing broad CI from re-running needlessly on superseded intermediate heads.
