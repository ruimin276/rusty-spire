---
id: SPEC-005
title: Combat Initialization and Transition Semantics
status: accepted
domain: combat
version: 1
applies_to: v0.3
depends: [SPEC-003, SPEC-004]
sources: [crates/rusty-spire-combat/src/setup.rs, crates/rusty-spire-combat/src/engine.rs, crates/rusty-spire-data/src/catalog.rs, packages/spire-codex-stable-v0.107.1.json]
---

# SPEC-005: Combat Initialization and Transition Semantics

## Status

ACCEPTED

## Summary

`rusty-spire-combat` validates and initializes the supported single-enemy combat slice, enumerates every legal decision, and applies immutable deterministic transitions whose ordering, RNG consumption, damage rules, and content capabilities are defined here.

## Specification

### Public boundary

The implementation is `Simulator<'a>` in
`crates/rusty-spire-combat/src/engine.rs`; `CombatEngine<'a>` is its public type
alias. The accepted entry points are the following current signatures:

```rust
pub fn initialize(
    &self,
    setup: &CombatSetupV1,
    allow_debug_rng_overrides: bool,
) -> Result<InitializedCombat, SimulatorError>;

pub fn legal_actions(&self, state: &CombatState) -> Result<Vec<Action>, SimulatorError>;
pub fn step(&self, state: &CombatState, action: &Action)
    -> Result<CombatState, SimulatorError>;
pub fn state_id(&self, state: &CombatState) -> Result<String, SimulatorError>;
```

`state_hash` is the v0.2-compatible name for `state_id`. Both validate the
snapshot and return BLAKE3 over the dedicated canonical combat-state DTO in
SPEC-003. `step` clones its input before mutation; successful and failed branches
therefore do not mutate their parent state.

### CMB-001 — Validated deterministic initialization

```text
CombatSetupV1 + selected package
        │ validate identity, capability, HP, ids, relics, encounter, RNG
        ▼
derive named RNG streams ── monster_ai ──► omitted enemy HP
        │
        ▼
expand deck → assign instance ids → construct turn-1 state
        │
        ▼
shuffle full draw pile ── shuffle ──► opening hand
        │
        ▼
InitializedCombat { state, setup_hash, policy }
```

| Rule | Owner / Where | Why |
|---|---|---|
| Package SHA-256 MUST match before state construction | `validate_setup` | Prevent mixed data and semantics |
| Character HP MUST satisfy `0 < current_hp <= max_hp` | setup validation | Invalid actors cannot enter combat |
| Deck MUST be non-empty; quantities MUST be positive | setup validation | Every card instance is explicit |
| Only upgrade levels 0 and 1 are executable | setup and snapshot validation | Only base/upgraded values exist |
| Potions and stateful relic inputs MUST be rejected | setup validation | Their mechanics are not implemented |
| Exactly one enemy MUST be selected | setup and snapshot validation | Target ordering is single-enemy only |
| Unknown ids and unsupported mechanics MUST fail closed | all validation paths | Data presence is not capability |

`CombatSetupV2` is converted by `rusty-spire-api` to this validated setup using
its explicit `{package_id, sha256}` identity. A catalog encounter is executable
only when it contains one enemy. A custom enemy may override positive current
and maximum HP, with current HP no greater than maximum HP. Otherwise maximum HP
is rolled inclusively from the normal range, or the ascension range at level 8+.

Deck entries expand in setup order. Each instance receives the decimal string of
its zero-based expansion index. Cost comes from the package; `Exhaust` and
`Ethereal` keywords set instance flags. Character energy defaults to the package
value unless a positive override is supplied. Relics become stateless model
instances. Enemy combat ids are `enemy_0`; its opening move and ascension flags
come from the package.

The only accepted profile is `isolated_combat_xoshiro_v1`, using
`xoshiro256_star_star_v1` and `numeric_seed_domain_v1`. The run seed MUST parse as
a decimal `u32`. Twelve named streams are reconstructed; `shuffle` uses the run
seed directly and every other stream uses deterministic domain derivation.
Overrides are allowed only behind the explicit debug flag and MUST name an
existing stream. Enemy HP consumes `monster_ai`; card order consumes `shuffle`.

Combat starts at turn 1, side `Player`, decision `PlayerAction`, full deck in
`draw_pile`, and all other piles empty. Preparation performs descending
Fisher-Yates using `shuffle`, then draws to `5 + additional_opening_draw` cards.
Ring of the Snake therefore opens with seven cards. `setup_hash` is SHA-256 of
the serialized validated setup.

### CMB-002 — Legal actions and ordered card effects

Legal actions and card effects **MUST** follow this requirement's validation and
ordering rules.

Before every action query or transition, the engine validates snapshot schema 2,
base unmodded provenance, supported character/content, ascension flags, RNG
algorithm and shuffle stream, unique non-empty card instance ids, powers,
decision shape, and the one-enemy boundary.

| Decision | Legal actions |
|---|---|
| `PlayerAction` | Every affordable playable card, plus `end_turn` |
| `CardSelection` | One choice per candidate for an exact one-card discard |
| `Terminal` | None |

A card is playable when it is not Ascender's Bane, its effective cost is
non-negative, and cost does not exceed current energy. A declaration containing
`damage` or `apply_power` is targeted and produces one action per living enemy;
other cards are untargeted. `step` accepts only an action id returned by
`legal_actions`; all other supplied fields are replaced by that canonical action.

Card resolution order is:

1. remove the selected instance from hand;
2. pay effective energy cost;
3. append the card to `play_pile`;
4. translate declared effects in package order into a FIFO queue;
5. execute the queue in order;
6. resolve a required one-card discard choice, if any;
7. move a nonlethal resolved card to `exhaust_pile` when `exhausts`, otherwise
   to `discard_pile`.

The closed effect vocabulary is:

| Declaration | Runtime effect | Target |
|---|---|---|
| `damage` | Modified attack damage, then block/HP application | Living enemy |
| `block` | Add amount to player block | Player |
| `draw` | Draw until hand grows by amount | Player |
| `energy` | Add amount to current energy | Player |
| `apply_power` | Add amount to an existing or new power stack | Living enemy |
| `discard` | After primary effects, branch over exactly one hand card | Player hand |

Amounts select `base` at upgrade 0 and `upgraded` at upgrade 1. Declarations are
never inferred from description text. A lethal damage effect does not interrupt a
later block, draw, or energy entry. A later `apply_power` observes that all enemies
are dead and stops the remaining queue; a later damage entry currently fails because
its target is no longer living. Promoted v0.3 declarations contain no repeated
post-lethal damage sequence. After the queue, a final-enemy kill removes dead enemies
and marks the state terminal. In the current v0.3 representation, that lethal resolving card
remains in `play_pile`; changing this observable pile state requires a spec and
fixture update.

Damage is computed once using integer arithmetic:

```text
raw = base + attacker Strength
raw *= Weak(3/4), then player-only Shrink(7/10), then defender Vulnerable(3/2)
damage = max(0, floor(combined numerator / combined denominator))
blocked = min(block, damage); block -= blocked; hp = max(0, hp - damage + blocked)
```

All active multipliers are combined before the single integer division.
Strength and like power stacks add amounts. Weak and Vulnerable are durations;
Shrink and Strength do not tick each turn.

Draw removes index 0 from `draw_pile`. When empty, a non-empty discard pile is
first sorted with the .NET-compatible introspective ordering by `(model_id,
upgrade_level)`, shuffled with the same `shuffle` stream, appended to draw, and
then consumed. If both piles are empty, drawing stops without error. RNG counters
are part of state identity, so identical branches reconstruct identical draws.

### CMB-003 — End-turn, enemy, and terminal ordering

`end_turn` **MUST** perform the following sequence without an intermediate player
decision:

1. Drain hand in order. Ethereal cards and Ascender's Bane exhaust; retained
   cards stay in hand; all others discard.
2. For each living enemy, clear its block and execute its current move in the
   package order `damage`, then `block`, then `power`.
3. Replace the intent with `next_move` and append that next move to history.
4. Tick player Weak/Vulnerable, then enemy Weak/Vulnerable; remove zero stacks.
5. If player HP is zero, mark terminal and stop.
6. Otherwise clear player block, reset energy to maximum, increment turn, set
   side to `Player`, and draw until hand size is five.

Enemy damage uses the same damage pipeline but never applies player-only Shrink.
Level 8 selects ascension HP and tough-enemy block values; level 9 selects
deadly-enemy damage and power values. A transition is won when every enemy has
HP zero and lost when player HP is zero. Either condition sets `Decision::Terminal`.
A win removes player Shrink; a loss preserves the post-attack terminal snapshot.
No legal actions exist after terminal status.

**PROHIBITED:**

- executing descriptions, unknown effects, unsupported powers, or arbitrary ids;
- multiple executable enemies, potions, gameplay mods, or non-base revisions;
- consuming RNG outside the named state-held streams;
- continuing to a new turn after lethal enemy damage;
- mutating an input snapshot while exploring another branch.

### CMB-004 — Complete v0.3 content capability matrix

The executable v0.3 content set **MUST** be limited to this matrix. All listed cards
support base and upgraded forms; arrows are execution order.

| Card | Character / cost | Base → upgraded behavior |
|---|---|---|
| Adrenaline | Silent / 0 | energy 1→2 → draw 2 → exhaust |
| Ascender's Bane | Neutral / -1 | unplayable; exhaust during hand cleanup |
| Backflip | Silent / 1 | block 5→8 → draw 2 |
| Bash | Ironclad / 2 | damage 8→10 → Vulnerable 2→3 |
| Defend | Ironclad / 1 | block 5→8 |
| Defend | Silent / 1 | block 5→8 |
| Iron Wave | Ironclad / 1 | block 5→7 → damage 5→7 |
| Neutralize | Silent / 0 | damage 3→4 → Weak 1→2 |
| Pommel Strike | Ironclad / 1 | damage 9→10 → draw 1→2 |
| Strike | Ironclad / 1 | damage 6→9 |
| Strike | Silent / 1 | damage 6→9 |
| Survivor | Silent / 1 | block 8→11 → choose and discard 1 |

| Capability group | Executable content and behavior |
|---|---|
| Characters | Ironclad, Silent; package energy, caller-supplied valid HP |
| Powers | Strength, Weak, Vulnerable, Shrink |
| Relics | Ring of the Snake: +2 opening draw; Burning Blood and Winged Boots: inert |
| Single encounters | Nibbits Weak, Fuzzy Wurm Crawler Weak, Shrinker Beetle Weak |
| Represented but rejected | Nibbits Normal and Overgrowth Crawlers: multiple enemies |

| Enemy | HP normal / A8+ | Deterministic move cycle; base / A9+ values |
|---|---|---|
| Nibbit | 42–46 / 44–48 | Butt damage 12/13 → Slice damage 6/7 + block 5/6 → Hiss Strength self 2/3 → Butt |
| Fuzzy Wurm Crawler | 55–57 / 58–59 | First Acid Goop damage 4/6 → Inhale Strength self 7/7 → Acid Goop damage 4/6 → First |
| Shrinker Beetle | 38–40 / 40–42 | Shrinker applies player Shrink 1/1 → Chomp damage 7/8 → Stomp damage 13/14 → Chomp |

This matrix is exhaustive for the committed v0.3 package and supported claims. The
generic engine accepts any character-compatible package card with a non-empty closed-
vocabulary effect list, so an alternate valid package can execute additional
combinations. Such combinations are outside conformance and MUST NOT be advertised as
supported until this specification and its tests are amended.

## Conformance

| Requirement | Automated evidence | Required assertions |
|---|---|---|
| CMB-001 | `test:combat_initialize`; `initializes_stable_instances_and_rng_vectors`; `identical_setup_replays_opening_actions_and_hash`; `rejects_unknown_ids_and_multi_enemy_execution` | Identity, validation, stream vectors, HP roll, shuffle, and opening hand are deterministic |
| CMB-002 | `test:effect_validation`; `survivor_resolves_its_discard_as_a_branchable_choice`; `hash_distinguishes_rng_counters`; `branch_order_cannot_change_repeated_transition` | Legal actions, effect FIFO, choices, piles, damage, and RNG state conform |
| CMB-003 | `test:combat_transitions`; `neutralize_weak_reduces_the_next_enemy_attack`; `fuzzy_wurm_crawler_uses_its_fixed_scaling_cycle`; `nibbit_cycle_and_ascenders_bane_are_exact` | Cleanup, moves, power ticks, next turn, and terminal stop follow the specified order |
| CMB-004 | `test:proof_slice_cards`; `supported_mechanics_match_the_pinned_spire_codex_contract`; `composable_effect_cards_follow_reviewed_order_and_upgrades`; `catalog_drives_ascension_moves_and_inert_relics` | Every matrix row and both upgrade levels use promoted values and reviewed ordering |

An accepted content row MUST have promoted evidence, package validation, an
engine handler, and a conformance assertion. A package-only record is not enough.

## References

- [SPEC-003: Combat Domain and State Invariants](003-domain.md)
- [SPEC-004: Spire Codex Evidence and Data Packages](004-data.md)
- [Pinned v0.107.1 data package](../packages/spire-codex-stable-v0.107.1.json)
- [Reviewed effect declarations](../packages/reviewed-effects-v1.json)
- [Combat engine](../crates/rusty-spire-combat/src/engine.rs)
- [Setup initialization](../crates/rusty-spire-combat/src/setup.rs)
- [Traceability manifest](traceability.json)
