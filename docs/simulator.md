# Deterministic Combat Simulator

The simulator is a standalone Rust implementation of the supported STS2 combat
rules. STS2 is required only when capturing a snapshot or recording a
differential trace. Solving and comparing captured scenarios do not load the
game, its assemblies, or its assets.

## Fidelity contract

Simulator snapshots are accepted only at a stable player decision. Every
future-relevant field is state: ordered piles, card instance identity, enemy
move state/history, powers, turn state, and each named RNG stream. `step` is
pure: exploring another branch cannot change the result of repeating an earlier
transition.

Unknown mechanics fail with `unsupported mechanic`; they are never treated as
no-ops. The legacy STS2 `v0.107.0` adapter uses `dotnet_system_random_v1`,
matching that build's seeded `System.Random` and counter restoration. The
checked-in fixture and current local `v0.108.0` build use
`xoshiro256_star_star_v1`; that adapter is verified against a bridge-generated
golden vector. The RNG implementation is selected by each snapshot rather than
forking the combat engine.

Bridge 0.2.1 no longer identifies an RNG from its private class name alone. It
runs a fixed seed-1/four-value probe and emits a supported algorithm id only
when the game matches a checked vector. Otherwise snapshots contain
`unverified_rng_<fingerprint>` and fail closed. This matters because Spire
Codex's extraction manifests show changes to the shared RNG source between
v0.108.0 and later beta builds even though the high-level entity data remains
stable.

The snapshot records both `game_version` and the exact `assembly_sha256`.
Content changes and RNG changes are separate compatibility dimensions.
When present, `combat.ascension_level` is validated against every enemy's
`tough_enemies` and `deadly_enemies` flags (Tough at 8+, Deadly at 9+). Older
fixtures may omit the field, but new bridge captures always include it.

## Reverse-engineered source contract

`tests/fixtures/spire_codex_supported_content_v0110.json` is a small, reviewed
projection of the public Spire Codex stable and beta feeds. It pins the cards,
relics, monster values, HP ranges, and parsed state-machine identifiers used by
the supported slice, plus published extraction-manifest fingerprints for key
decompiled files. Run:

```bash
.venv/bin/python scripts/verify_spire_codex_contract.py
```

to compare the contract with the latest local stable and beta snapshots. Rust
tests execute supported cards and enemy attacks using values read from that
checked-in contract, so a source refresh and a simulator behavior change meet
in one test boundary.

The contract is evidence for static values, not executable parity. Spire
Codex's parser does not fully encode power-duration timing, effect ordering,
custom commands, RNG consumption, rounding, or every ascension-dependent power
and block amount. Those remain gated by direct code inspection or live golden
traces. In particular, a parsed fixed power amount is not enough to remove an
existing ascension branch without stronger evidence.

## Snapshot schema v2

A scenario selects the native engine with:

```json
{
  "oracle": {"type": "simulator"},
  "initial_state": {
    "snapshot_schema": 2,
    "provenance": {
      "game_version": "v0.108.0",
      "game_commit": "58694f64",
      "assembly_sha256": "...",
      "content_revision": "base",
      "modded_gameplay": false
    },
    "rng": {
      "algorithm": "xoshiro256_star_star_v1",
      "run_seed": "...",
      "streams": {
        "shuffle": {"seed": 123, "counter": 17}
      }
    }
  }
}
```

The complete shape is represented by the Rust types in
`crates/sls2-combat-core/src/state.rs`. State hashes cover the entire typed
snapshot, including RNG counters and array order.

Actions retain the bridge-compatible ids:

- `play:<card-instance>:<target-combat-id>` for attacks.
- `play:<card-instance>` for self-targeted cards.
- `end_turn` for ending the player turn.
- `choose:<choice-id>:<candidate>` for a single-card modal decision.

## Supported mechanics slice

Implemented in the current slice (see the live-versus-offline evidence boundary
below):

- Ironclad Strike, Defend, Bash, and Ascender's Bane.
- Silent Strike, Defend, Neutralize, Survivor, and Ring of the Snake's
  seven-card opening hand.
- Damage, block, energy, targeting, Vulnerable, Weak, Shrink, and Strength.
- Ordered hand/draw/discard/exhaust/play piles.
- Ethereal exhaustion, end-turn processing, draw, and reshuffle.
- Nibbit's Butt, Slice, and Hiss state-machine cycle, including ascension
  values, block, and Strength.
- Fuzzy Wurm Crawler's Acid Goop/Inhale cycle and Shrinker Beetle's
  Shrinker/Chomp/Stomp cycle.
- Survivor's one-card discard decision as an explicit branchable decision.
- Seeded .NET and xoshiro256** RNG reconstruction, Fisher-Yates, and the .NET
  card sort used before a reshuffle.
- Native uniform-cost search and baseline/candidate comparison.

Deliberately rejected for now:

- Healing cards and non-monotonic HP objectives.
- Potions, gameplay mods, combat-active relics, and other cards/enemies/powers.
- Character-specific subsystems and multiplayer.
- Modal choices other than Survivor's single-card discard.
- Runtime creation of cards or entities and their next-instance-id counters;
  the supported slice never creates either during combat.
- Partially executing action queues.

Burning Blood and Winged Boots are accepted only because the simulation ends
at combat victory and neither changes this supported combat slice.

## Capture and differential traces

Build/install bridge 0.2.1, launch STS2 through Steam, and enter a supported
combat. Capture only when the player can make a choice and no effects are
resolving:

```bash
sls2-combat capture --output scenario.json
```

For a reproducible development fixture, restart STS2 at the main menu and run:

```bash
sls2-combat debug-start-nibbit \
  --allow-live-mutation \
  --output scenario.json
```

Record one real transition for comparison:

```bash
sls2-combat trace-step \
  --scenario scenario.json \
  --action-id 'play:3:enemy_0' \
  --allow-live-mutation \
  --output trace.json
```

The trace contains before/after snapshots, legal actions, per-stream RNG
counter deltas, and the game's checksums. `/export_state` and `/live_step`
remain available for legacy workflows.

The checked-in v0.108.0 golden cases cover an ordinary block card, player/enemy
turn boundaries, Ethereal exhaust, a counter-advancing reshuffle, Bash and
Vulnerable damage, and Nibbit's complete move cycle.

## Offline Silent weak-encounter matrix

The single-enemy portion of the Overgrowth easy pool is Nibbit, Fuzzy Wurm
Crawler, and Shrinker Beetle. The fourth easy encounter, Group of Slimes, is
excluded because it is a multi-enemy fight. Run the Silent starter deck matrix
with:

```bash
.venv/bin/python scripts/run_silent_weak_matrix.py \
  --seeds 1,2,4,7,17,42 \
  --output tests/fixtures/silent_weak_seed_matrix.json
```

These scenarios use Ascension 0, the midpoint of each published HP range, and
feed the listed shuffle seed directly to the verified xoshiro adapter. They are
source-backed offline tests, not live differential fixtures: the local STS2
installation was unavailable when this slice was added. The older Ironclad
fixtures remain the only cases verified against a concrete game assembly.

A wider sweep of shuffle seeds 1 through 100 completed and won all 300 cases:

| Enemy | Zero-HP-loss seeds | Worst optimal loss | Maximum explored states |
|---|---:|---:|---:|
| Nibbit | 67 / 100 | 9 | 23,285 |
| Fuzzy Wurm Crawler | 68 / 100 | 8 | 13,835 |
| Shrinker Beetle | 84 / 100 | 9 | 4,072 |

Search collapses choices between mechanically identical starter-card copies;
their instance IDs differ, but no mechanic in this slice can distinguish them.
This keeps the exact action/state API while removing factorial duplicate search
branches.

## Development and verification

```bash
python3 -m venv .venv
.venv/bin/python -m pip install -e .
cargo test --workspace
.venv/bin/python -m unittest discover -s tests -v
bridge/build_bridge.sh "/path/to/Slay the Spire 2"
.venv/bin/python scripts/benchmark_simulator.py
```

With STS2 restarted at the main menu, the complete native optimum can be
differentially replayed through the real game:

```bash
.venv/bin/python scripts/replay_live_solution.py --allow-live-mutation
```

The checked-in v0.108.0 acceptance replay contains 14 actions, matches at every
decision boundary, loses 2 HP, and wins at 78 HP.

The performance gate expands exactly 100,000 states and requires completion in
under five seconds on the development Mac. Differential fixtures remain keyed
by game assembly hash; a fixture from another build is evidence for that build,
not a blanket compatibility claim.
