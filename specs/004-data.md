---
id: SPEC-004
title: Spire Codex Evidence and Data Packages
status: accepted
domain: data
version: 2
applies_to: stable v0.107.1 evidence, promotion, and DataPackageV1
depends: [SPEC-001, SPEC-002]
sources: [tools/spire-codex/fetch.py, tools/spire-codex/sync.py, tools/spire-codex/promote.py, tools/spire-codex/verify.py, evidence/spire-codex-stable-v0.107.1-selected.json, packages/reviewed-effects-v1.json, catalogs/combat_v0.107.1.json, packages/spire-codex-stable-v0.107.1.json, crates/rusty-spire-data/src/catalog.rs]
---

# SPEC-004: Spire Codex Evidence and Data Packages

## Status

ACCEPTED

## Summary

Rusty Spire turns immutable Spire Codex observations, a reviewed behavior
selection, and the retained v0.2 catalog into one deterministic offline
`DataPackageV1`, while keeping static evidence separate from executable rules.

## Specification

The active data flow is deliberately layered:

```text
manual network access
    -> immutable full snapshot under data/upstream/       (not committed)
    -> selected Spire Codex evidence under evidence/      (committed)
                 + reviewed behavior/assets under packages/
                 + retained v0.2 catalog under catalogs/
    -> deterministic DataPackageV1 under packages/        (committed)
    -> rusty-spire-data validation                         (runtime/CI, offline)
```

The terms in this specification are distinct:

| Term | Meaning |
|---|---|
| Source snapshot | Timestamped raw/normalized responses fetched from one Codex channel |
| Selected evidence | Compact committed records used by the v0.3 package |
| Reviewed behavior | Human-selected effect vocabulary/order, ownership, and assets |
| Base catalog | Retained v0.2 reviewed definitions and rules |
| Package bytes | Exact UTF-8 JSON bytes loaded and identified at runtime |
| `source.content_sha256` | Upstream content identity carried from the base source metadata |
| `CombatCatalog.sha256` | SHA-256 of the exact package or legacy-catalog input bytes |

### DAT-001 — Layered evidence

Executable data **MUST** preserve the authority boundary between Spire Codex
observations and reviewed gameplay semantics.

| Information | Authoritative input | Not authoritative |
|---|---|---|
| Selected model IDs/names and card numeric values | Selected Spire Codex evidence | UI copies or prose documentation |
| Retained monster HP and move values | Base catalog plus combat fixtures | Selected evidence supplies identity/name/asset only |
| Card effect order and effect kind | `packages/reviewed-effects-v1.json` plus accepted combat specs/tests | Natural-language card descriptions |
| Retained powers, encounters, RNG profiles, ascension rules, modifiers | `catalogs/combat_v0.107.1.json` plus conformance fixtures | Newly fetched records alone |
| Runtime capability | Data validation plus accepted combat semantics | Presence of an ID in the package |
| Assets selected for the Web package | Reviewed asset mapping tied to evidence records | Ad hoc Web constants |

`tools/spire-codex/promote.py` reads numeric fields such as `damage`, `block`,
`cards_draw`, and `energy_gain`; it does not parse a card description into
effects. Targeting, effect ordering, RNG consumption, enemy behavior, and power
semantics require accepted specifications and pinned conformance evidence.
Unknown or unreviewed effect names fail promotion rather than receiving a
best-effort interpretation.

The runtime effect vocabulary is the tagged enum in
`crates/rusty-spire-data/src/catalog.rs`:

```rust
pub enum CardEffectDefinition {
    Damage { amount: UpgradableValue },
    Block { amount: UpgradableValue },
    Draw { amount: UpgradableValue },
    Energy { amount: UpgradableValue },
    ApplyPower { id: String, amount: UpgradableValue },
    Discard { amount: usize },
}
```

Package presence does not imply executable support. In particular, encounter
records may describe more than one enemy while the v0.3 combat engine rejects
multi-enemy execution.

### DAT-002 — Immutable ingestion

Network fetching **MUST** produce channel-separated immutable snapshots, and
CI/runtime verification **MUST NOT** require network access.

`tools/spire-codex/fetch.py` is the only public network entry point. It defaults
to stable, requires `--channel beta` for beta, writes below
`data/upstream/`, and delegates crawling to `sync.py`.

| Fetch property | Required behavior |
|---|---|
| Channel | Stable and beta have separate paths and identities |
| Raw evidence | Preserve endpoint response bytes |
| Normalized evidence | Require arrays of objects, required fields, and unique string IDs |
| Provenance | Record endpoint hashes, aggregate content hash, version evidence, counts, and retrieval metadata |
| Existing identical content | Reuse the latest snapshot unless `--force` is explicit |
| Forced fetch | Create a new timestamped snapshot; never overwrite an existing snapshot |
| Rate limiting | Delay requests and retry the supported rate-limit response |
| Failure | Exit nonzero before publishing a partial/invalid snapshot |

Full snapshots are local evidence and are ignored by Git. Promotion and
verification use committed inputs and Python's standard library. CI may test
the fetcher with mocked responses, but it must not contact Spire Codex. In v0.3,
normalization is a minimum-shape check: unknown fields and many non-ID field types are
preserved rather than rejected, so it MUST NOT be described as full schema validation.

### DAT-003 — Deterministic promotion

Promotion **MUST** combine the three committed input classes below and emit
byte-for-byte deterministic package JSON without interpreting descriptions.

| Input | Default path | Contribution |
|---|---|---|
| Selected evidence | `evidence/spire-codex-stable-v0.107.1-selected.json` | Card numeric values; monster/relic identity and names |
| Reviewed selection | `packages/reviewed-effects-v1.json` | Exact selected ID set, ordered effects, character/type metadata, assets |
| Retained base | `catalogs/combat_v0.107.1.json` | Source provenance, retained definitions, powers, encounters, RNG/ascension/modifier rules |

`promote(evidence_path, review_path, base_path)` requires evidence and review
schema version 1 and exact equality between reviewed and evidenced card ID
sets. Missing monster/relic evidence, unknown reviewed characters, missing
numeric values, invalid upgrade deltas, missing reviewed power mappings, or an
unknown effect name raises `PromotionError`.

The emitted JSON uses `json.dumps(..., indent=2, sort_keys=True)` followed by
one newline. Map order and whitespace are therefore deterministic for the
three input values. `tools/spire-codex/verify.py` regenerates the expected bytes
in memory from those defaults and compares them directly with
`packages/spire-codex-stable-v0.107.1.json`; a difference is a stale-package
failure. Verification is offline and does not rewrite the package.

Promotion is not a general content compiler in v0.3: it does not discover
arbitrary Codex records, merge beta content, infer effects, or prove combat
support. Changing any of the three inputs requires reviewed package output and
conformance changes in the same change.

### DAT-004 — Package identity

A loaded package **MUST** carry its manifest identity and the SHA-256 of the
exact bytes supplied to `CombatCatalog::from_json`; callers **MUST NOT** treat
that digest as a semantic/canonical-JSON hash.

The grouped on-disk contract is:

```rust
pub struct DataPackageV1 {
    pub manifest: DataPackageManifestV1,
    pub cards: BTreeMap<String, CardDefinition>,
    pub actors: ActorContentV1,
    pub items: ItemContentV1,
    pub encounters: BTreeMap<String, EncounterDefinition>,
    pub rules: RuleContentV1,
}

pub struct DataPackageManifestV1 {
    pub schema_version: u32,
    pub package_id: String,
    pub source: CatalogSource,
}
```

| Group | Contents |
|---|---|
| `cards` | Card definitions and ordered effect declarations |
| `actors` | `characters`, `monsters` |
| `items` | `relics`, `powers` |
| `encounters` | Encounter definitions |
| `rules` | `rng_profiles`, `ascensions`, `combat_modifiers` |

Every package struct denies unknown JSON fields. Loading a grouped package
requires manifest schema version 1 and a non-empty package ID, flattens it into
the current `CombatCatalogV1` runtime shape, and applies catalog validation.
That validation includes:

| Category | Rejected condition |
|---|---|
| Source | Channel is not stable or beta |
| Required groups | RNG profiles, characters, cards, powers, or monsters are empty |
| RNG | Algorithm/derivation is not the supported v1 adapter |
| Cards | Referenced power is absent, or damage/block/draw/energy amount is negative |
| Powers | Stack behavior is neither `amount` nor `duration` |
| Monsters | HP range is invalid, opening/next move is absent, or power target/reference is invalid |
| Encounters | Enemy list is empty or references an unknown monster |
| Rules | Ascension thresholds or combat modifier ratios are invalid |

The runtime wrapper is currently:

```rust
pub struct CombatCatalog {
    pub data: CombatCatalogV1,
    pub sha256: String,
    pub package_id: String,
}

pub type DataPackage = CombatCatalog;
```

`sha256` is `SHA256(input_bytes)`. Reformatting otherwise equivalent JSON
changes it. The deterministic promoter provides the repository's one stable
encoding, but the loader does not canonicalize arbitrary input first.
`source.content_sha256` identifies upstream source content and is not the same
digest. `CombatSetupV2` names both `manifest.package_id` and the raw-byte
package digest; both must equal the loaded package or validation returns a
package-mismatch error.

For v0.3 compatibility, `CombatCatalog::from_json` also accepts the flat legacy
`CombatCatalogV1`, derives a package ID from its game version, promotes the
reviewed legacy card effects in memory, and still hashes the original input
bytes. That compatibility path does not change the grouped package contract.

Current validation checks an `apply_power` target ID but does not reject a negative
power amount. It also accepts any `discard` amount, while combat creates a selection
only for exactly one discard. Promoted v0.3 content uses positive power amounts and
`discard: 1`; broader values are represented but are not executable guarantees.

## Conformance

| Requirement | Observable acceptance criterion | Registered verification |
|---|---|---|
| DAT-001 | Promotion uses selected static fields and reviewed effect order; proof-slice output contains the reviewed declarations | `test:data_evidence` |
| DAT-002 | Mocked stable/beta fetches preserve raw/normalized/provenance data and reject overwrite, drift, duplicates, and rate-limit failures | `test:spire_codex_fetch` |
| DAT-003 | Two promotions from the same three inputs are identical and offline verification byte-compares the committed package | `check:data_verify` |
| DAT-004 | Rust loads the grouped package, computes its raw-input SHA, validates references/effects, and rejects invalid package content | `test:data_package` |

## References

- [SPEC-001: Project Scope and Correctness Policy](001-project.md)
- [SPEC-002: Architecture and Crate Boundaries](002-architecture.md)
- [SPEC-005: Combat Initialization and Transition Semantics](005-combat.md)
- [Data source guide](../docs/data_sources.md)
- [Spire Codex tool guide](../tools/spire-codex/README.md)
- [Selected stable evidence](../evidence/spire-codex-stable-v0.107.1-selected.json)
- [Reviewed effect selection](../packages/reviewed-effects-v1.json)
- [Retained v0.2 catalog](../catalogs/combat_v0.107.1.json)
- [Generated stable package](../packages/spire-codex-stable-v0.107.1.json)
