# Slay the Spire 2 Isolated Combat Simulator

This repository contains a pure Rust simulator and optimal-search CLI for one
isolated Slay the Spire 2 combat. It does not launch the game, simulate map or
reward progression, or perform network access at runtime.

The active workspace has two crates:

- `sls2-combat-core`: catalog loading, strict setup validation, named RNG
  streams, combat mechanics, state transitions, hashing, policy, and search.
- `sls2-combat-cli`: the `sls2-combat` command.

Python is limited to the independent Spire Codex crawler and reviewed catalog
build tools in `tools/spire_codex/`. The retired live-game bridge and its golden
traces are retained in `archive/sts2-bridge/` and are excluded from Cargo.

## Quick start

```bash
cargo build --release

target/release/sls2-combat catalog-info \
  --catalog catalogs/combat_v0.107.1.json

target/release/sls2-combat validate \
  --catalog catalogs/combat_v0.107.1.json \
  --input fixtures/combat_setup_v1/silent_nibbit_seed_1.json

target/release/sls2-combat solve \
  --catalog catalogs/combat_v0.107.1.json \
  --input fixtures/combat_setup_v1/silent_nibbit_seed_1.json
```

Compare two combat setups:

```bash
target/release/sls2-combat compare \
  --catalog catalogs/combat_v0.107.1.json \
  --baseline fixtures/combat_setup_v1/silent_nibbit_seed_1.json \
  --candidate fixtures/combat_setup_v1/silent_fuzzy_seed_4.json
```

`minimize_hp_loss` is the default and only v0.2 policy. A winning result is
optimal only when `optimality_proven` is true. Time, state, and turn limits
always return `optimality_proven: false`.

## Supported v0.2 slice

- Ironclad and Silent starter combat cards.
- Ring of the Snake; explicitly cataloged non-combat relics are inert.
- Nibbit, Fuzzy Wurm Crawler, and Shrinker Beetle.
- Strength, Weak, Vulnerable, and Shrink.
- One enemy at execution time, deterministic draw/reshuffle, card upgrades,
  Survivor discard selection, block, energy, powers, and ascension values.

Potions, healing, multiple enemies, special character resources, and unknown
combat-active entities fail closed. Combat ends immediately on victory or
defeat, so rewards and post-combat effects do not exist in the runtime model.

See [simulator mechanics and schemas](docs/simulator.md) and
[data ingestion and promotion](docs/data_sources.md).

## Verification

```bash
cargo test --workspace
python3 -m unittest discover -s tools/spire_codex/tests -v

# Dedicated release-mode performance gate: exactly 100,000 explored states,
# under five seconds.
cargo test --release -p sls2-combat-core --test silent_weak_matrix \
  expands_one_hundred_thousand_states_under_five_seconds -- --ignored
```
