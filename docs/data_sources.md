# Spire Codex ingestion and promotion

The normative data policy is [`SPEC-004`](../specs/004-data.md).

`tools/spire-codex/fetch.py` captures immutable stable or beta snapshots with
raw bytes, response metadata, endpoint hashes, aggregate identity, and version
evidence. Full snapshots remain ignored and CI never uses the network.

The runtime package is produced from three committed inputs:

- a compact, source-pinned evidence selection in `evidence/`;
- reviewed executable effect ordering and presentation metadata in `packages/`;
- the previous reviewed v0.2 catalog for retained mechanics evidence.

Regenerate and verify it offline:

```bash
python3 tools/spire-codex/promote.py
python3 tools/spire-codex/verify.py
```

The generated `DataPackageV1` has a manifest and five content groups: cards,
actors, items, encounters, and rules. Promotion verifies that every selected
card is present in the compact evidence and emits deterministic sorted JSON.

Spire Codex is authoritative for identities and represented static values. It
is not treated as executable code: descriptions are never parsed into effects,
and effect order, targeting, RNG consumption, and parser gaps require separate
reviewed evidence and conformance tests.
