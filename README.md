# Slay the Spire 2 Combat Solver

This project is now focused on one task: solve a fixed Slay the Spire 2 combat
state for the winning line with minimum HP loss.

The Python solver does not reimplement card, relic, enemy, or RNG mechanics. It
delegates exact state transitions to an oracle backed by the running game. A mock
oracle is included so the solver and CLI can be tested before the game mod bridge
is available.

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

Check a running game bridge:

```bash
sls2-combat oracle-health
```

Export the current live combat from the game bridge:

```bash
sls2-combat export --output scenario.json
```

Important limits:

```bash
--max-states 100000
--max-turns 50
--timeout-seconds 60
```

If a limit is hit, the result has `complete: false`; it is not an optimality claim.

## Scenario Format

The schema is intentionally flexible because the game oracle owns exact mechanics.
The solver requires only enough state to evaluate terminal status, HP loss, and
tie-breakers.

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
- C# STS2 bridge mod source under `bridge/`
- current-combat export and oracle health endpoints
- macOS Steam mod-loader path discovery
- STS2 settings helper for enabling the local bridge mod
- scenario solve CLI
- baseline/candidate comparison CLI
- tests for solver behavior and CLI output

Still required for real STS2 accuracy:

- launch STS2 from Steam, confirm the bridge initializer log, and export one
  live combat state
- implement branchable `/step` using STS2 combat state clone/restore
- manual acceptance test by replaying the returned action sequence in-game

See [docs/sts2_oracle_spike.md](docs/sts2_oracle_spike.md) for the local
read-only install inspection and bridge implications.
See [bridge/README.md](bridge/README.md) for bridge build/install commands.
