---
id: SPEC-001
title: Project Scope and Correctness Policy
status: accepted
domain: product
version: 1
applies_to: v0.3
depends: [SPEC-000]
sources: [Cargo.toml, README.md]
---

# SPEC-001: Project Scope and Correctness Policy

## Status

ACCEPTED

## Summary

Rusty Spire is an offline, deterministic Slay the Spire 2 combat engine whose supported claims are limited to behavior backed by accepted specifications, promoted evidence, and conformance tests.

## Specification

### Applicability and terminology

This specification governs every active Rust crate, application, generated contract,
fixture, and data package. In this specification:

| Term | Meaning |
|---|---|
| **supported** | Accepted behavior with an executable conformance check |
| **represented** | Data can be stored but may not be executable |
| **exact** | Search has no approximate pruning and follows SPEC-006 proof rules |
| **offline** | Application runtime and gameplay-data verification do not contact network services |
| **fail closed** | Reject unsupported input before returning a simulated result |

The source-of-truth precedence is:

```text
accepted specifications
        ↓ constrain
reviewed evidence + promoted data package
        ↓ configure
Rust implementation + generated contracts
        ↓ exposed by
CLI / WASM / Web
```

Explanatory documentation and README files cannot override this chain.

### PRJ-001 — Product boundary

| Rule | Owner / Where | Why |
|---|---|---|
| The product MUST simulate isolated combat deterministically | active Rust crates | Replays must be auditable |
| Native and browser clients MUST use the same Rust engine | `rusty-spire-api`, WASM | Prevent semantic drift |
| Search MUST obtain legal actions and successors through combat APIs | `rusty-spire-simulator` | One transition authority |
| Runtime operation MUST remain local and offline | CLI, WASM, Web | Reproducible and private |

Search MAY inspect immutable state fields for objectives, ordering, terminal results,
and traces; it MUST NOT implement an alternate transition. The project is a combat
analysis toolkit, not a general game clone. A valid run starts
from a complete combat setup, initializes one isolated fight, explores or executes
legal actions, and terminates with a win, loss, proof, or explicit incomplete result.

**PROHIBITED:**

- silently querying live Spire Codex data at runtime;
- implementing a second combat engine in TypeScript, Python, CLI, or WASM glue;
- describing a result as game-perfect outside the explicitly supported slice;
- using wall-clock performance as a correctness claim.

### PRJ-002 — Active v0.3 capability slice

The executable v0.3 capability boundary **MUST** match this table:

| Capability | v0.3 support | Governing spec |
|---|---|---|
| Package-bound combat setup | Supported | SPEC-004, SPEC-005 |
| Deterministic initialization and transitions | Supported | SPEC-003, SPEC-005 |
| One player and one executable enemy | Supported | SPEC-005 |
| Ironclad and Silent content in the committed package | Supported when validated | SPEC-004, SPEC-005 |
| Exact HP-loss search and setup comparison | Supported | SPEC-006 |
| CLI, JSON, WASM dispatcher, worker, static Web | Supported | SPEC-007, SPEC-008 |
| Multi-enemy encounter records | Represented only | SPEC-004 |
| Archived bridge traces | Historical evidence only | SPEC-009 |

The executable content set is the intersection of:

1. records in the selected `DataPackageV1`;
2. declarations accepted by package validation;
3. mechanics implemented by `rusty-spire-combat`;
4. setup constraints accepted by initialization; and
5. behavior covered by the traceability manifest.

Presence in Spire Codex or in a package alone does not imply executable support.

### PRJ-003 — Explicit exclusions

| Excluded capability | Required behavior today | Promotion gate |
|---|---|---|
| Run progression, map, rewards, shops | Reject or omit | New accepted domain spec |
| Potions | Reject non-empty setup | Data, combat, API, and test contracts |
| Multiple executable enemies | Reject before execution | Targeting/order specification |
| Live game mutation or automation | Never execute | Accepted replacement for SPEC-009 |
| Network-backed runtime data | Never fetch | New product and security specification |
| Arbitrary mods/content revisions | Reject | Provenance and semantics review |
| Approximate optimality claims | Never claim proof | SPEC-006 amendment |

Future-facing types MAY reserve fields for excluded capabilities. Reserved fields MUST
either be empty/default or produce a stable unsupported error; they MUST NOT trigger
partial behavior.

### PRJ-004 — Correctness and failure policy

| Input condition | Required outcome | Error surface |
|---|---|---|
| Malformed JSON or unknown fields | Reject | `invalid_json` / `invalid_request` |
| Package identity mismatch | Reject before initialization | `package_mismatch` |
| Unknown model identifier | Reject | `unknown_id` |
| Known but unsupported mechanic | Reject | `unsupported` |
| Illegal action for current decision | Reject without mutation | `invalid_action` |
| Search resource limit reached | Return explicit incomplete result | termination reason + no proof |
| Internal invariant failure | Error; never fabricate result | `internal` or Rust error |

Every successful combat validation, solve, or comparison output MUST be attributable
to a package identity, setup identity, and deterministic transition path. Unsupported
behavior cannot be approximated merely to keep a solve running.

#### Change discipline

Any change to observable behavior MUST update, in the same pull request:

1. the accepted requirement that authorizes the behavior;
2. its entry in `traceability.json` when verification changes;
3. at least one conformance test or generated-contract check; and
4. compatibility documentation when a public contract changes.

New functionality starts as a DRAFT specification if implementation and conformance
are not yet complete. It becomes ACCEPTED only when the behavior exists and all
mapped checks pass.

## Conformance

| Requirement | Automated evidence | Review evidence |
|---|---|---|
| PRJ-001 | `check:architecture`, `test:cli_legacy` | No duplicate runtime engine |
| PRJ-002 | `test:workspace`, `test:web` | Capability table matches package/code |
| PRJ-003 | `test:unsupported_content` | Reserved fields remain fail-closed |
| PRJ-004 | `test:unsupported_content`, `test:api_errors` | Errors do not fabricate results |

A reviewer MUST reject an accepted capability claim that lacks a mapped check or that
depends on network access during CI.

## References

- [SPEC-000: Specification Format and Governance](000-specification-guidelines.md)
- [SPEC-004: Spire Codex Evidence and Data Packages](004-data.md)
- [SPEC-005: Combat Initialization and Transition Semantics](005-combat.md)
- [SPEC-006: Exact Search and Proof Semantics](006-search.md)
- [Traceability manifest](traceability.json)
- [Project README](../README.md)
