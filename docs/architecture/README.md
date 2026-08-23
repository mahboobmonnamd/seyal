# Seyal Architecture

This directory is the canonical entry point for Seyal foundation architecture.

## Read in this order

1. [`SEYAL-ARCH-FOUNDATION-RD-001.md`](SEYAL-ARCH-FOUNDATION-RD-001.md) — canonical foundation architecture and Milestone-001 decision package.
2. [`rationale/SEYAL-ARCH-FOUNDATION-RATIONALE-001.md`](rationale/SEYAL-ARCH-FOUNDATION-RATIONALE-001.md) — reasons, rejected alternatives, failure modes, and revisit conditions for foundation decisions and prohibitions.
3. [`ui/SEYAL-UI-ARCHITECTURE-001.md`](ui/SEYAL-UI-ARCHITECTURE-001.md) — presentation architecture for Flow/Raw/TUI, history, Blocks, workspace chrome, inspectors, attention/approvals, desktop/mobile continuity, and render priority.
4. [`source/FOUNDATION-RD-BRIEF.md`](source/FOUNDATION-RD-BRIEF.md) — source requirements that initiated this architecture pass.

## Authority

- The foundation architecture document is the **canonical decision document**.
- The rationale document explains **why** each rule exists. It does not create a competing architecture.
- The UI architecture is subordinate to terminal/runtime ownership and performance invariants.
- The source brief records requirements/research questions; it is not an implementation specification.
- Future ADRs may refine a foundation decision only when they cite the affected rationale ID and provide new evidence, measurements, or platform constraints.

## Change discipline

Do not create competing `-v2`, `-final`, `-new`, or similarly duplicated architecture copies. Amend the canonical document and preserve architectural history in ADR/rationale updates.

Repository changes should be made through **branch → pull request → review/validation → merge**.
