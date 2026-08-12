---
id: SPEC-004
title: Spire Codex evidence and data packages
status: accepted
depends: [SPEC-001, SPEC-002]
---

# Spire Codex evidence and data packages

### DAT-001 — Layered evidence

Spire Codex is authoritative for content identity and represented static
values. Executable semantics, effect order, targeting, and RNG consumption
require an accepted spec plus pinned game, bridge-trace, or decompiled-source
evidence.

### DAT-002 — Immutable ingestion

Fetching publishes immutable stable or beta snapshots with endpoint hashes,
aggregate identity, provenance, and schema validation. CI performs no network
fetches.

### DAT-003 — Deterministic promotion

Promotion consumes one immutable snapshot and a reviewed behavior selection,
then emits a deterministic `DataPackageV1` and compact committed evidence.
Descriptions are never interpreted as executable behavior.

### DAT-004 — Package identity

A package has a stable package ID, schema version, source provenance, and
SHA-256 of its canonical bytes. Combat setups must name both package ID and
hash.
