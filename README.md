# Slay the Spire 2 Combat Solver

This project is now focused on one task: solve a fixed Slay the Spire 2 combat
state for the winning line with minimum HP loss.

The fast path is now a standalone Rust combat simulator and native search
engine. It implements a strict subset of card, enemy, pile, and RNG mechanics
without launching STS2. The game-backed bridge remains the source
of high-fidelity snapshots and differential traces, and the mock oracle remains
available for solver tests.

## Solver Model

V1 scope:

| area | behavior |
|---|---|
| combat | one fixed single-player combat |
| objective | win while minimizing HP loss |
| search | uniform-cost search ordered by HP loss |
| potions | excluded from legal actions |
| healing | not modeled; HP loss is monotonic for supported scenarios |
| mechanics | resolved by the oracle, not Python |

Tie-breakers among equal-HP-loss states are deterministic and heuristic:

1. more powers played
2. lower total enemy HP
3. more retained block
4. fewer actions

Only HP loss defines optimality. Tie-breakers affect which equal-cost policy is
returned first.

## CLI

Run a mock scenario:

```bash
PYTHONPATH=src python3 -m sls2_combat_solver.cli solve \
  --scenario examples/mock_candidate.json
```

Compare two complete scenario variants:

```bash
PYTHONPATH=src python3 -m sls2_combat_solver.cli compare \
  --baseline examples/mock_baseline.json \
  --candidate examples/mock_candidate.json
```

After installing the package, use:

```bash
sls2-combat solve --scenario scenario.json --output result.json
sls2-combat compare --baseline baseline.json --candidate candidate.json --output report.json
```

Run the native simulator fixture:

```bash
python3 -m venv .venv
.venv/bin/python -m pip install -e .
.venv/bin/sls2-combat solve \
  --scenario examples/sim_nibbit.json \
  --max-states 1000000
```

Run the Silent starter deck against every single-enemy Overgrowth opening
encounter across several shuffle seeds:

```bash
.venv/bin/python scripts/run_silent_weak_matrix.py \
  --seeds 1,2,4,7,17,42 \
  --output tests/fixtures/silent_weak_seed_matrix.json
```

This matrix is offline/source-backed rather than live-differential because the
game installation was unavailable when those cases were added. See
`docs/simulator.md` for the exact assumptions and evidence boundary.

Check a running game bridge:

```bash
sls2-combat oracle-health
```

Export the current live combat from the game bridge:

```bash
sls2-combat export --output scenario.json
```

Capture a self-contained native-simulator scenario at a stable player decision:

```bash
sls2-combat capture --output scenario.json
```

Record a game-resolved transition for differential testing:

```bash
sls2-combat trace-step --scenario scenario.json \
  --action-id 'play:3:enemy_0' --allow-live-mutation --output trace.json
```

Important limits:

```bash
--max-states 100000
--max-turns 50
--timeout-seconds 60
```

If a limit is hit, the result has `complete: false`; it is not an optimality claim.

## Scenario Format

Mock and HTTP scenarios remain intentionally flexible. Simulator scenarios use
the strict snapshot schema v2 documented below because hidden RNG and enemy state
must participate in branching and deduplication.

Minimum shape:

```json
{
  "oracle": {"type": "http", "base_url": "http://127.0.0.1:17351"},
  "initial_state": {
    "player": {"hp": 70, "block": 0},
    "combat": {"won": false, "lost": false, "turn": 1},
    "metrics": {"powers_played": 0},
    "enemies": [{"id": "enemy_0", "hp": 42}]
  }
}
```

Real game exports should also include game/build version, mod version, deck zones,
card ids/upgrades/modifiers, relic state, enemy intents, and RNG/draw state. Those
fields are passed back to the oracle unchanged.

## Oracle Contract

The real STS2 mod bridge is expected to expose these HTTP endpoints:

### `POST /legal_actions`

Request:

```json
{"state": {...}}
```

Response:

```json
{"actions": [{"id": "play_card:hand_0:enemy_0", "type": "card"}]}
```

Potion actions may be returned by the oracle, but the solver filters
`{"type": "potion"}` in v1.

### `POST /step`

Request:

```json
{"state": {...}, "action": {...}}
```

Response:

```json
{"state": {...}}
```

The returned state must be the exact game-resolved state after applying the
action, including enemy turns, draw/discard changes, relic counters, powers, and
RNG state.

### `POST /state_hash`

Request:

```json
{"state": {...}}
```

Response:

```json
{"state_hash": "canonical-state-id"}
```

The hash must identify all combat-relevant state. If two states can lead to
different future outcomes, they must not share a hash.

## Current Implementation Status

Implemented:

- uniform-cost combat solver
- deterministic tie-breakers
- mock oracle
- HTTP oracle client
- standalone Rust simulator and native uniform-cost search
- immutable branch states with named RNG stream state
- v0.107.0 seeded-.NET and current xoshiro RNG adapters, Fisher-Yates, and
  reshuffle ordering
- Ironclad starter-card and Nibbit mechanics slice
- C# STS2 bridge mod source under `bridge/`
- legacy export plus high-fidelity simulator snapshot and trace endpoints
- macOS Steam mod-loader path discovery
- STS2 settings helper for enabling the local bridge mod
- scenario solve CLI
- baseline/candidate comparison CLI
- tests for solver behavior and CLI output

Still required before expanding the supported content set:

- add each new card/enemy/power behind an exact trace or golden mechanic test
- add a new RNG adapter before accepting snapshots from any future PRNG change

Committed bridge-derived golden cases cover Defend, end turn, Ascender's Bane
exhaustion, deterministic reshuffle, Bash/Vulnerable, Strike, and Nibbit's
Butt/Slice/Hiss cycle. The native 14-action optimum has also been replayed
through the live bridge with exact state equality at every decision, finishing
at 78 HP.

See [docs/sts2_oracle_spike.md](docs/sts2_oracle_spike.md) for the local
read-only install inspection and bridge implications.
See [bridge/README.md](bridge/README.md) for bridge build/install commands.
See [docs/simulator.md](docs/simulator.md) for the v2 snapshot contract,
supported mechanics, and verification workflow.
See [docs/data_sources.md](docs/data_sources.md) for the upstream source policy
and the provenance-rich cards/relics/enemies snapshot crawler.

Fetch the current remote catalog without downloading artwork:

```bash
.venv/bin/python scripts/fetch_game_data.py
```

Verify the simulator's checked-in supported-content projection against those
local stable and beta snapshots:

```bash
.venv/bin/python scripts/verify_spire_codex_contract.py
```
