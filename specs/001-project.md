---
id: SPEC-001
title: Project purpose and scope
status: accepted
depends: []
---

# Project purpose and scope

### PRJ-001 — Product

Rusty Spire is a deterministic, offline, isolated Slay the Spire 2 combat
engine and exact-search toolkit. Native and browser applications must execute
the same Rust combat and search implementations.

### PRJ-002 — Active scope

Version 0.3 supports combat setup, deterministic state transitions, legal
actions, state identifiers, exact search, comparison, CLI usage, and a static
local-first Web application.

### PRJ-003 — Exclusions

Run progression, map traversal, rewards, multiple-enemy execution, potions,
network runtime dependencies, and bridge-driven live mutation are unsupported
unless promoted by a later accepted specification.

### PRJ-004 — Failure policy

Unknown, unreviewed, or unsupported gameplay content must fail closed with a
stable error category and must never be approximated silently.
