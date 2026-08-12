---
id: SPEC-009
title: Archived STS2 Evidence Bridge Reactivation
status: draft
domain: bridge-security
version: 1
applies_to: future
depends: [SPEC-001, SPEC-003, SPEC-004, SPEC-007]
sources: [archive/sts2-bridge/README.md, archive/sts2-bridge/Sls2CombatOracle.Bridge/OracleServer.cs, archive/sts2-bridge/Sls2CombatOracle.Bridge/ICombatBridge.cs]
---

# SPEC-009: Archived STS2 Evidence Bridge Reactivation

## Status

DRAFT

## Summary

This non-binding proposal defines the evidence, security, compatibility, and review gates that a future STS2 bridge must satisfy before any archived bridge code can re-enter the active project.

## Specification

### Current archive status

`archive/sts2-bridge` is historical evidence only. It is outside the Cargo workspace,
is not compiled or executed by CI, and is not a runtime dependency. The archived C#
server binds `127.0.0.1:17351`, exports state/actions/RNG information, and also
contains guarded live-mutation/checkpoint endpoints. It has no session authentication,
request-size limit, or complete detached branchable-step implementation. Nothing in
this draft authorizes running, installing, or modifying it.

The requirements below are proposed reactivation conditions. They are non-binding
while this specification remains DRAFT and receive no traceability mappings.

### BRI-001 — Keep the bridge outside active v0.3 products

Until an accepted successor lands, active crates, CLI commands, WASM, Web, data
promotion, and CI MUST NOT compile, install, launch, or call the archived bridge.
Archive files MAY be read as historical context, but their behavior is neither a
supported API nor conformance evidence by itself.

Reactivation would require moving reviewed code out of `archive/` in a dedicated
change. That move must not silently preserve old endpoint contracts or installation
helpers as supported interfaces.

### BRI-002 — Treat captures as versioned evidence, not semantics

A reactivated bridge would emit immutable evidence bundles containing:

| Evidence field | Required identity or constraint |
|---|---|
| Game build | Version plus executable/assembly fingerprint |
| Bridge | Contract version and build revision |
| Content | Channel/revision and relevant model IDs |
| State | Versioned snapshot before and after action |
| Action | Legal-action set and selected stable action identity |
| RNG | Algorithm fingerprint, named stream state, consumed outputs |
| Transition | Ordered trace, checksum/state identity, terminal result |
| Environment | Ascension and gameplay-affecting modifiers; no account data |

Captured observations MAY corroborate accepted combat rules. They MUST NOT promote
new executable semantics automatically, infer mechanics from descriptions, or
override accepted specifications. Conflicts require quarantine, reproduction, and a
reviewed spec/data/test amendment.

### BRI-003 — Require a least-privilege local security boundary

Any successor server would be required to:

| Control | Proposed requirement |
|---|---|
| Binding | Loopback addresses only; no wildcard or LAN listener |
| Authentication | Per-session unguessable capability token on every request |
| Methods and content | Explicit POST routes, JSON content type, bounded bodies |
| Read behavior | Detached export by default; no game mutation |
| Mutation | Separate build/feature plus explicit per-request user consent |
| Data minimization | Never collect credentials, tokens, account IDs, saves, or unrelated files |
| Logging | Exclude request bodies and sensitive local paths by default |
| Lifetime | Stop listener and invalidate token when the game/mod unloads |

The archived listener's loopback binding and `allow_live_mutation` boolean are useful
precedents but are insufficient: absence of authentication and body limits blocks
reactivation. Mutation endpoints must never be used by routine CI, Web clients, or an
unattended optimizer.

### BRI-004 — Satisfy explicit reactivation gates

Before this draft can be replaced by an accepted specification, one review must
provide all of the following:

1. a threat model covering local hostile processes, browser requests, logs, and game
   save integrity;
2. a versioned bridge protocol with schemas, stable errors, compatibility policy, and
   game-build rejection behavior;
3. detached read/export behavior and proof that unsupported reflection fails closed;
4. authentication, request bounds, cancellation/timeouts, and opt-in mutation tests;
5. sanitized golden evidence for at least the accepted proof slice;
6. parity tests showing how bridge evidence corroborates, but does not control, Rust;
7. installation/removal documentation with backups and no silent settings mutation;
8. an architecture and CI amendment that keeps live-game tests isolated and optional;
9. explicit maintainer approval of legal, security, and maintenance implications.

Failure of any gate keeps the bridge archived. A game update or fingerprint mismatch
must disable capture until compatibility is reviewed; class or method names alone are
not evidence of compatibility.

## Conformance

These statements describe evidence expected by a future acceptance review; they are
not current CI gates and do not authorize execution of the archive.

| Requirement | Proposed acceptance evidence | Current v0.3 state |
|---|---|---|
| BRI-001 | Architecture check excludes bridge from active build graph | Archived and inactive |
| BRI-002 | Versioned sanitized bundles plus parity-review procedure | Historical captures only |
| BRI-003 | Security tests for binding, token, bounds, logging, mutation consent | Controls incomplete |
| BRI-004 | Accepted replacement spec and reviewed gate checklist | Not satisfied |

## References

- [SPEC-001: Project Scope and Correctness Policy](001-project.md)
- [SPEC-003: Combat Domain and State Invariants](003-domain.md)
- [SPEC-004: Spire Codex Evidence and Data Packages](004-data.md)
- [Archived bridge README](../archive/sts2-bridge/README.md)
- [Archived HTTP server](../archive/sts2-bridge/Sls2CombatOracle.Bridge/OracleServer.cs)
- [Archived bridge interface](../archive/sts2-bridge/Sls2CombatOracle.Bridge/ICombatBridge.cs)
