# Rusty Spire

[![CI](https://github.com/ruimin276/rusty-spire/actions/workflows/ci.yml/badge.svg)](https://github.com/ruimin276/rusty-spire/actions/workflows/ci.yml)

Rusty Spire v0.3 is a deterministic, offline Slay the Spire 2 isolated-combat
engine, exact-search toolkit, CLI, and local-first Web application. Development
is governed by the accepted requirements in [`specs/`](specs/README.md); CI
checks their traceability, generated contracts, crate boundaries, data-package
reproducibility, behavior, builds, and coverage. Repository tasks are routed
for coding agents through [`AGENTS.md`](AGENTS.md).

## Architecture

The active Rust workspace has explicit one-way boundaries. Here `A -> B` means
“A depends on B”:

```text
cli -> api <- wasm <- web
api -> {core, data, combat, simulator, heuristics}
heuristics -> {core, simulator}
simulator -> {core, data, combat}
combat -> {core, data}
data -> core
core -> {}
```

- `rusty-spire-core`: state, decisions, actions, typed IDs, RNG, canonical IDs.
- `rusty-spire-data`: versioned packages, static content, provenance, validation.
- `rusty-spire-combat`: setup validation, initialization, legal actions, rules.
- `rusty-spire-simulator`: objectives, exact search, comparison, proof semantics.
- `rusty-spire-heuristics`: optional ordering heuristics.
- `rusty-spire-api`: shared v0.3 DTOs, service, stable errors, v0.2 adapters.
- `apps/cli`: the `rusty-spire` executable.
- `rusty-spire-wasm` and `apps/web`: versioned browser ABI and static UI.
- `tools/spire-codex`: immutable fetch, reviewed promotion, offline verification.

`archive/sts2-bridge` remains historical evidence and is not built or executed.
Its possible future role is described only by the draft bridge specification.

## Quick start

```bash
cargo build --workspace --release

target/release/rusty-spire validate \
  --catalog catalogs/combat_v0.107.1.json \
  --input fixtures/combat_setup_v1/silent_nibbit_seed_1.json

target/release/rusty-spire solve \
  --catalog catalogs/combat_v0.107.1.json \
  --input fixtures/combat_setup_v1/silent_nibbit_seed_1.json
```

The v0.2 file contracts are accepted during v0.3 with deprecation notices and
are removed in v0.4. New integrations should use `CombatSetupV2`,
`SolveRequestV1`, `ContentManifestV1`, and the `sls2_call_v1` dispatcher.

Run the browser application:

```bash
cd apps/web
npm ci
npm run dev
```

The Web application gets package identity, characters, starter decks, cards,
enemies, relics, and assets from `ContentManifestV1`; TypeScript does not
duplicate catalog values or combat mechanics.

## Supported v0.3 slice

The existing Ironclad/Silent starter slice, three single enemies, supported
powers and starter relic behavior are preserved. The reviewed composable-effect
slice adds Iron Wave, Backflip, Pommel Strike, and Adrenaline, including both
upgrade levels and declared effect order.

The reviewed [implementation ledger](specs/content/implemented-v1.json) and
generated [publication ledger](specs/content/published-v1.json) show exactly which
card and relic mechanics are realized and which are distributed in the stable
package.

Potions, multiple-enemy execution, map/reward/run progression, gameplay mods,
and unreviewed content fail closed.

## Verification

```bash
python3 tools/specs/check.py
python3 tools/specs/generate_contracts.py --check
python3 tools/specs/generate_content_status.py --check
python3 tools/specs/check_architecture.py
python3 tools/spire-codex/verify.py
python3 -m unittest discover -s tools/specs/tests -v

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked --no-fail-fast
python3 -m unittest discover -s tools/spire-codex/tests -v

cd apps/web
npm ci
npm test
```

See [`docs/simulator.md`](docs/simulator.md) and
[`docs/data_sources.md`](docs/data_sources.md) for explanatory material. When
they disagree with an accepted spec, the spec is authoritative.
