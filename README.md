# Slay the Spire 2 Isolated Combat Simulator

[![CI](https://github.com/ruimin276/rusty-spire/actions/workflows/ci.yml/badge.svg)](https://github.com/ruimin276/rusty-spire/actions/workflows/ci.yml)

This repository contains a pure Rust simulator and optimal-search CLI for one
isolated Slay the Spire 2 combat. It does not launch the game, simulate map or
reward progression, or perform network access at runtime.

The active workspace has three crates:

- `rusty-spire-core`: catalog loading, strict setup validation, named RNG
  streams, combat mechanics, state transitions, hashing, policy, and search.
- `rusty-spire-cli`: the `rusty-spire` command.
- `rusty-spire-wasm`: a small browser ABI that embeds the reviewed catalog and
  runs the same Rust initialization and optimal search in a Web Worker.

Python is limited to the independent Spire Codex crawler and reviewed catalog
build tools in `tools/spire_codex/`. The retired live-game bridge and its golden
traces are retained in `archive/sts2-bridge/` and are excluded from Cargo.

The portable static application in `web/` provides an interactive combat
workbench and compiles the Rust engine to WebAssembly. Searches execute in a
client-side Web Worker; the host only serves static files. JavaScript handles
the interface and ABI transport but does not reimplement combat mechanics.

## Quick start

```bash
cargo build --release

target/release/rusty-spire catalog-info \
  --catalog catalogs/combat_v0.107.1.json

target/release/rusty-spire validate \
  --catalog catalogs/combat_v0.107.1.json \
  --input fixtures/combat_setup_v1/silent_nibbit_seed_1.json

target/release/rusty-spire solve \
  --catalog catalogs/combat_v0.107.1.json \
  --input fixtures/combat_setup_v1/silent_nibbit_seed_1.json
```

Run the web interface:

```bash
cd web
npm install
npm run dev
```

The build requires the `wasm32-unknown-unknown` Rust target. Run `npm run build`
and upload `web/dist/` to any static host.

Compare two combat setups:

```bash
target/release/rusty-spire compare \
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

CI exposes three required checks. Run their equivalents locally after adding
the formatting, Clippy, coverage, and WebAssembly components:

```bash
rustup component add rustfmt clippy llvm-tools-preview
rustup target add wasm32-unknown-unknown
```

### Test

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo clippy --target wasm32-unknown-unknown -p rusty-spire-wasm --locked -- -D warnings
cargo test --workspace --all-features --locked --no-fail-fast
python3 -m unittest discover -s tools/spire_codex/tests -v

cd web
npm ci
npm test
cd ..

# Dedicated release-mode performance gate: exactly 100,000 explored states,
# under five seconds.
cargo test --locked --release -p rusty-spire-core --test silent_weak_matrix \
  expands_one_hundred_thousand_states_under_five_seconds -- --ignored
```

### Coverage

Rust coverage is enforced at 85% lines. Install the pinned tool once, then run:

```bash
cargo install cargo-llvm-cov --version 0.8.7 --locked
cargo llvm-cov \
  --package rusty-spire-core \
  --package rusty-spire-cli \
  --all-features \
  --locked \
  --fail-under-lines 85
```

### Build

```bash
cargo build --workspace --release --locked
target/release/rusty-spire --version

cd web
npm ci
npm run check:wasm-fingerprint
RUSTY_SPIRE_WASM_PATH=public/rusty_spire_wasm.wasm \
  node --test tests/wasm-solver.test.mjs
npm run build
cd ..

test -f web/dist/index.html
test -f web/dist/rusty_spire_wasm.wasm
git diff --exit-code -- web/public/rusty_spire_wasm.sources.sha256
```

Rust can emit different raw WebAssembly bytes on different host architectures.
The portable source fingerprint covers every Rust, catalog, lockfile, and build
script input to the module, while the committed module is smoke-tested directly
before the current platform rebuilds it.
