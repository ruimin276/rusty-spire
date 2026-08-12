---
id: SPEC-003
title: Combat domain and state invariants
status: accepted
depends: [SPEC-002]
---

# Combat domain and state invariants

### DOM-001 — Branch isolation

Every transition returns a new state. A branch must not mutate its input or
consume RNG from another branch.

### DOM-002 — Identity and decisions

Card instance IDs are non-empty and unique across all piles. Non-terminal
states expose exactly one supported decision boundary.

### DOM-003 — Snapshot contract

The v0.3 domain snapshot is represented by `CombatSnapshotV3`. Its stable
state ID is BLAKE3 over canonical JSON with recursively sorted object keys and
preserved array order, independent of Rust field declaration order.

### DOM-004 — RNG

RNG algorithm identifiers, named stream seeds, and counters are part of the
state. Unsupported algorithms and missing required streams fail closed.
