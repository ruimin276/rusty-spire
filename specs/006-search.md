---
id: SPEC-006
title: Search, objectives, and heuristics
status: accepted
depends: [SPEC-003, SPEC-005]
---

# Search, objectives, and heuristics

### SRCH-001 — Default objective

The default objective minimizes combat-start HP minus final HP. Deterministic
tie-breakers may select among equal results but must not change the objective.

### SRCH-002 — Exact mode

Exact search is the default. A winning result is proven optimal only when the
frontier rule proves it; an exhausted frontier proves no winning line.

### SRCH-003 — Limits and approximation

Resource-limit termination is incomplete. Approximate pruning requires an
explicit approximate mode and can never return `optimality_proven: true`.

### SRCH-004 — Heuristic contract

The simulator accepts a heuristic interface. Zero and admissible heuristics
must preserve exact results and action-enumeration independence.
