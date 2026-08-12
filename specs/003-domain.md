---
id: SPEC-003
title: Combat Domain and State Invariants
status: accepted
domain: domain
version: 2
applies_to: v0.3 combat state, identity, decisions, and RNG
depends: [SPEC-002]
sources: [crates/rusty-spire-core/src/state.rs, crates/rusty-spire-core/src/id.rs, crates/rusty-spire-core/src/rng.rs, crates/rusty-spire-core/src/canonical.rs, crates/rusty-spire-combat/src/engine.rs, crates/rusty-spire-combat/src/setup.rs, crates/rusty-spire-api/src/lib.rs]
---

# SPEC-003: Combat Domain and State Invariants

## Status

ACCEPTED

## Summary

Combat state is an immutable-branching, explicitly versioned value whose
supported decision forms, two distinct BLAKE3 identity contracts, and named
reconstructible RNG streams are validated before engine use.

## Specification

`CombatState` in `crates/rusty-spire-core/src/state.rs` is the engine's complete
branch value:

```rust
pub struct CombatState {
    pub snapshot_schema: u32,
    pub provenance: Provenance,
    pub rng: RngBankState,
    pub combat: CombatStatus,
    pub decision: Decision,
    pub player: PlayerState,
    pub enemies: Vec<EnemyState>,
    pub hand: Vec<CardInstance>,
    pub draw_pile: Vec<CardInstance>,
    pub discard_pile: Vec<CardInstance>,
    pub exhaust_pile: Vec<CardInstance>,
    pub play_pile: Vec<CardInstance>,
    pub metrics: Metrics,
}
```

Serde rejects unknown fields on the state and its nested structs. Successful
deserialization is only structural; a state becomes an engine state after
`CombatEngine::validate_state` applies the supported-content and decision
checks below.

### DOM-001 — Branch isolation

Every successful engine transition **MUST** return a new `CombatState` without
mutating its input state or sharing RNG consumption with another branch.

The public transition signature enforces an immutable input borrow:

```rust
pub fn step(
    &self,
    state: &CombatState,
    action: &Action,
) -> Result<CombatState, SimulatorError>;
```

`step` validates the input and action, clones the state, applies the action to
the clone, and returns that clone. Combat preparation follows the same pattern.
Because every RNG stream's seed and counter are fields of the clone, advancing
one result cannot advance its sibling or its parent.

| Observable invariant | Required result |
|---|---|
| Replay the same valid state/action | Same resulting state and RNG counters |
| Execute sibling actions in either order | Each branch result is unchanged |
| Receive an invalid action or state | No successor is returned; input is unchanged |
| Clone and resume a stream at counter `n` | Next value equals uninterrupted draw `n` |

Wall-clock search timing and allocation identity are not combat state and are
not covered by deterministic transition equality.

### DOM-002 — Identity and decisions

An engine-approved state **MUST** use unique non-empty card instance IDs across
all five piles and exactly one supported `Decision` variant at a time.

The current decision type is a tagged enum:

```rust
pub enum Decision {
    PlayerAction,
    CardSelection {
        choice_id: String,
        candidates: Vec<String>,
        min: usize,
        max: usize,
    },
    Terminal,
}
```

`CombatEngine::validate_state` applies these current v0.3 constraints:

| Surface | Accepted invariant | Failure category |
|---|---|---|
| Snapshot | `snapshot_schema == 2`, base content, unmodded gameplay | invalid snapshot or unsupported mechanic |
| Cards | Instance ID is non-empty and globally unique across hand/draw/discard/exhaust/play | invalid snapshot |
| Cards | Definition is known, belongs to the player when character-bound, has a reviewed handler, and upgrade is 0 or 1 | unknown ID or unsupported mechanic |
| Active combat | Non-terminal states have `current_side == "Player"` and exactly one supported enemy | invalid snapshot or unsupported mechanic |
| Terminal combat | Zero or one enemy is accepted; legal actions are empty | unsupported mechanic for more than one enemy |
| Player action | The enum itself represents the single decision boundary | invalid action if a submitted action is not legal |
| Card selection | Only `discard:<played-instance>`, `min == max == 1`, a resolving play-pile card, and non-empty hand-derived candidates are supported | invalid snapshot or unsupported mechanic |
| Content | Potions are empty; relic, enemy, move, and power IDs are known and supported | unknown ID or unsupported mechanic |
| Ascension | When `ascension_level` is present, stored tough/deadly flags agree with package thresholds | invalid snapshot |

`ModelId::new` accepts `NAMESPACE.NAME` where both non-empty components contain
only uppercase ASCII letters, digits, or underscores. Current v0.2-compatible
state structs still store model IDs as `String`; the typed helper does not
retroactively validate every deserialized string. Catalog-backed engine
validation is therefore authoritative for current state content.

When `ascension_level` is absent, `validate_state` does not check the tough/deadly
flags. The validator also does not independently normalize numeric fields, require
actor combat IDs to be unique, or prove that a supplied `Decision::Terminal` agrees
with the won/lost flags. Engine-created states maintain those conditions through
initialization and transition rules, but arbitrary snapshots must not infer stronger
guarantees than the checks above.

### DOM-003 — Snapshot contract

State identity **MUST** use the algorithm associated with its public surface;
the v0.3 snapshot ID and the compatibility engine state ID are distinct and
**MUST NOT** be substituted for one another.

| Identity | Public entry point | Hashed payload and serialization | Purpose |
|---|---|---|---|
| Canonical value ID | `rusty_spire_core::state_id<T>` | Compact JSON after recursively sorting object keys; arrays preserved | Generic versioned DTO identity |
| Snapshot V3 ID | `CombatSnapshotV3::state_id` | Canonical value ID of `{schema_version: 3, state: ...}` | Public v0.3 snapshot identity |
| Engine combat state ID | `CombatEngine::state_id` / `combat_state_id` | Compact Serde JSON of a dedicated fixed-order `CanonicalCombatState` DTO | v0.2 state-hash compatibility and search deduplication |

The API wrapper is:

```rust
pub struct CombatSnapshotV3 {
    pub schema_version: u32,
    pub state: CombatState,
}

pub fn state_id(&self) -> Result<String, ApiErrorV1>;
```

`CombatSnapshotV3::state_id` rejects wrapper versions other than 3, then hashes
the entire wrapper through generic canonical JSON. Object declaration order is
irrelevant to this algorithm; array order remains significant.

The engine algorithm intentionally does not construct and recursively sort a
generic JSON tree. It serializes these fields in this frozen order:

```text
snapshot_schema, provenance, rng, combat, decision, player, enemies,
hand, draw_pile, discard_pile, exhaust_pile, play_pile, metrics
```

That dedicated top-level DTO makes `CombatState` declaration reordering
irrelevant while preserving existing v0.2 hashes. Nested types retain their
Serde representation, and `BTreeMap` keys use map iteration order. Both
algorithms return the lowercase 64-hex-character BLAKE3 digest. They perform no
semantic normalization of arrays, strings, numbers, or equivalent game states.

### DOM-004 — RNG

All combat randomness **MUST** be reconstructible from the algorithm name and
the named stream's `{seed, counter}` stored in `CombatState`.

The state and primitive signatures are:

```rust
pub struct RngBankState {
    pub algorithm: String,
    pub run_seed: String,
    pub streams: BTreeMap<String, RngStreamState>,
}

pub struct RngStreamState {
    pub seed: u32,
    pub counter: u32,
}

pub fn next_int(
    algorithm: &str,
    stream: &mut RngStreamState,
    max_exclusive: u32,
) -> u32;
```

The supported profile is `xoshiro256_star_star_v1` with
`numeric_seed_domain_v1`. A decimal `u32` run seed initializes `shuffle`
directly. Every other named stream uses the first four digest bytes, decoded
little-endian, of:

```text
BLAKE3("sls2-combat-rng-domain-v1\0" || base_seed_le_u32 || stream_name_utf8)
```

Initialization creates these streams in a `BTreeMap`:

| Stream group | Names | Current v0.3 consumption |
|---|---|---|
| Shuffle | `shuffle` | Opening shuffle and discard reshuffles; `n - 1` draws for `n` cards |
| Monster setup | `monster_ai` | One draw when an omitted enemy HP value is rolled |
| Reserved parity domains | `up_front`, `unknown_map_point`, `combat_card_generation`, `combat_potion_generation`, `combat_card_selection`, `combat_energy_costs`, `combat_targets`, `niche`, `combat_orbs`, `treasure_room_relics` | Present and reconstructible; no active mechanic consumes them |

For each `next_int`, core reconstructs Xoshiro from the stream seed, advances
through `counter` raw values, generates one bounded value, and increments the
counter exactly once. The bound precondition is `0 < max_exclusive <= i32::MAX`.
Direct primitive callers must uphold that bound and pass the supported
algorithm; the primitive uses assertions/panic for programmer violations.
Setup initialization rejects an unsupported profile, non-numeric run seed, unknown
override, or missing derived streams before constructing combat. Snapshot validation
checks the stored algorithm and required `shuffle` stream but does not reparse
`run_seed`; subsequent transitions consume stored stream seeds and counters. Debug
seed overrides are accepted only through an explicitly enabled legacy/debug path.

Pinned reconstruction vectors include seed 1 values `[702, 520, 574, 391]`
under bound 1000, plus domain seeds `3114687082` for `shuffle` and `1831361556`
for `monster_ai`.

## Conformance

| Requirement | Observable acceptance criterion | Registered verification |
|---|---|---|
| DOM-001 | Repeated and reordered branches preserve their parent and reproduce state/RNG results | `test:branch_isolation` |
| DOM-002 | Model ID syntax and engine validation reject duplicate card instances, unsupported decisions, IDs, content, and multi-enemy execution | `test:core_invariants` |
| DOM-003 | Generic canonical JSON ignores object order, engine hashes distinguish RNG state, and engine hashes preserve v0.2 fixtures | `test:canonical_state_id` |
| DOM-004 | Xoshiro reconstruction, domain separation, seeded shuffle, HP rolls, and reshuffle counters match pinned vectors | `test:rng` |

## References

- [SPEC-002: Architecture and Crate Boundaries](002-architecture.md)
- [SPEC-005: Combat Initialization and Transition Semantics](005-combat.md)
- [SPEC-007: Versioned Application and Wire Interfaces](007-interfaces.md)
- [Core state definitions](../crates/rusty-spire-core/src/state.rs)
- [Canonical identity implementation](../crates/rusty-spire-core/src/canonical.rs)
- [RNG implementation](../crates/rusty-spire-core/src/rng.rs)
- [Combat state validation](../crates/rusty-spire-combat/src/engine.rs)
- [V2 setup fixtures](../fixtures/combat_setup_v2)
