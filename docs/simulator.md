# Simulator contract

## Runtime boundaries

`sls2-combat-core` is a deterministic, offline Rust library. A combat is
created only from a `CombatCatalogV1` file and a `CombatSetupV1` document. The
setup must contain the SHA-256 of the exact catalog bytes, preventing a data
refresh from silently changing an existing simulation.

The library exposes `initialize`, `Simulator::legal_actions`,
`Simulator::step`, `Simulator::state_hash`, `solve`, and `compare`. Every step
clones its input state. RNG counters are part of that state, so branch
exploration cannot mutate another branch.

The `sls2-combat-wasm` crate exposes the same `initialize` and `solve` path to a
static browser application through a minimal JSON ABI. The reviewed catalog is
embedded at compile time. Browser searches run in a Web Worker and import only
a monotonic clock from JavaScript; RNG, state transitions, hashing, policy, and
search all remain Rust code. The WebAssembly result preserves the normal
resource-limit and `optimality_proven` semantics.

## `CombatSetupV1`

The strict JSON schema is represented in
`crates/sls2-combat-core/src/setup.rs`. Unknown fields are rejected. Required
inputs are:

- `schema_version: 1` and the exact `catalog_sha256`;
- ascension level;
- decimal `run_seed` and a cataloged RNG `profile`;
- character ID, combat-start HP, maximum HP, and optional maximum energy;
- deck entries containing catalog ID, quantity, and upgrade level;
- relic IDs with empty state unless an override is explicitly supported;
- explicit `potions: []`;
- one tagged encounter: a catalog encounter ID or a custom enemy array;
- optional policy, defaulting to `minimize_hp_loss`.

The custom form can represent multiple enemies, but v0.2 returns
`unsupported mechanic` unless it contains exactly one. Omitted enemy HP is
rolled from the catalog through the named `monster_ai` stream. Explicit HP is
useful for contract fixtures and does not consume that roll.

Deck quantities expand in input order to stable instance IDs (`0`, `1`, ...)
before the opening shuffle. Relic effects then adjust the opening draw.

## RNG profile

The accepted profile is `isolated_combat_xoshiro_v1`. Its shuffle stream uses
the decimal `u32` run seed directly, preserving the verified shuffle vectors
and historical weak-enemy matrix. Other named streams are separated with the
versioned `numeric_seed_domain_v1` derivation.

This profile is intentionally named **isolated combat**: it is deterministic
and source-pinned, but it does not claim that a human-readable STS2 run seed is
fanned out exactly as the current game does. A future game-parity profile must
be added under a new ID with source-pinned derivation vectors. Per-stream seed
overrides are rejected unless the caller explicitly enables the debug/test
flag.

## Search and results

The only v0.2 policy is optimal graph search minimizing:

```text
combat-start HP - final HP
```

Healing is unsupported, so this cost is monotonic. Queue ordering uses
deterministic tie-breakers only after HP loss; tie-breakers never alter the
objective. The result includes catalog identity, setup hash, policy, win and
completion status, final HP, HP loss, action/state-hash trace, explored states,
cache hits, termination reason, and `optimality_proven`.

The first winning node removed from the loss-ordered frontier is optimal. A
fully exhausted frontier proves no winning line. `max_states`, `max_turns`, or
timeout termination is incomplete and never claims optimality.

## Supported mechanics and evidence

Executable behavior remains explicit Rust code keyed by catalog IDs; catalog
text never becomes executable logic. The verified slice covers Ironclad and
Silent starter combat cards, Ring of the Snake, three single enemies, and the
Strength/Weak/Vulnerable/Shrink powers. Static values, encounter composition,
ascension thresholds, modifiers, and move transitions live in the catalog.

`fixtures/contracts/spire_codex_supported_content_v1.json` records the
pinned Spire Codex values and extraction fingerprints used to review the slice.
Rust contract tests execute all promoted card upgrades, enemy attacks and
powers, relic behavior, modifier rounding, and ascension variants. The parser
is evidence for static data, not for effect ordering or RNG consumption; those
rules require decompiled-source vectors or archived game traces before
promotion.

The converted Silent weak-enemy matrix in `fixtures/evidence/` covers six seeds
for each supported enemy and asserts the same opening draws and optimal HP
outcomes under `CombatSetupV1`. The dedicated release benchmark expands exactly
100,000 states and enforces the five-second gate.
