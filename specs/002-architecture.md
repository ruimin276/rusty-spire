---
id: SPEC-002
title: Architecture and crate boundaries
status: accepted
depends: [SPEC-001]
---

# Architecture and crate boundaries

### ARC-001 — Dependency direction

The active Rust dependency graph is `core <- data <- combat <- simulator`,
with heuristics depending on simulator interfaces, API depending on active
library crates, and CLI/WASM depending on API. Reverse edges are forbidden.

### ARC-002 — Core boundary

`rusty-spire-core` owns domain state, actions, decisions, typed model IDs, RNG
state and algorithms, canonical snapshot state identifiers, and invariants. It
must not load catalogs, access files, search state graphs, or expose a platform
ABI.

### ARC-003 — Data and combat boundaries

`rusty-spire-data` owns versioned content packages and validation.
`rusty-spire-combat` owns setup validation, initialization, legal actions, and
deterministic state transitions.

### ARC-004 — Application boundaries

CLI and Web code may orchestrate the shared API but must not implement combat
rules. Network-aware Spire Codex code remains under `tools/spire-codex` and is
not a runtime dependency.
