---
id: SPEC-006
title: Exact Search and Proof Semantics
status: accepted
domain: search
version: 1
applies_to: v0.3
depends: [SPEC-003, SPEC-005]
sources: [crates/rusty-spire-simulator/src/search.rs, crates/rusty-spire-simulator/src/objective.rs, crates/rusty-spire-heuristics/src/lib.rs]
---

# SPEC-006: Exact Search and Proof Semantics

## Status

ACCEPTED

## Summary

`rusty-spire-simulator` explores the deterministic combat graph in nondecreasing incurred HP loss, deduplicates canonical states, and reports a proof only when exact traversal establishes an optimal win or exhausts every supported winning line without a resource cutoff.

## Specification

### Public boundary and graph model

The current public search interface is:

```rust
pub fn solve(
    catalog: &CombatCatalog,
    combat: &InitializedCombat,
    limits: SolveLimits,
) -> Result<SolveResult, SimulatorError>;

pub fn solve_with(
    catalog: &CombatCatalog,
    combat: &InitializedCombat,
    limits: SolveLimits,
    objective: &dyn CombatObjective,
    heuristic: &dyn Heuristic,
    mode: SearchMode,
) -> Result<SolveResult, SimulatorError>;

pub trait CombatObjective {
    fn kind(&self) -> ObjectiveKind;
    fn path_cost(&self, combat_start_hp: i32, current_hp: i32) -> i32;
}

pub trait Heuristic {
    fn estimate(&self, state: &CombatState) -> i32;
}
```

`solve` is exactly `solve_with(..., MinimizeHpLoss, ZeroHeuristic,
SearchMode::Exact)`. Search does not implement combat rules:

```text
InitializedCombat
      │ canonical state id
      ▼
 BinaryHeap frontier ── pop best node ──► terminal? ──► result/proof
      ▲                                      │ no
      │                                      ▼
      └── engine.step(state, representative legal action)
                 │
                 └── canonical child id + best-loss dedup + parent trace
```

A node owns a complete immutable `CombatState`, canonical hash, incurred HP
loss, parent index, incoming action, and NUL-separated action-id path key. Every
edge is a legal action followed by `CombatEngine::step`; RNG outcomes are
already deterministic state transitions, not stochastic search branches.

### SRCH-001 — Objective and deterministic ordering

The only accepted objective is `MinimizeHpLoss`:

```text
instant_cost(state) = combat_start_hp - state.player.hp
node_loss(child)    = max(parent.node_loss, instant_cost(child))
initial node_loss   = 0
```

The running maximum prevents later healing from erasing HP already lost. A
winning trace with smaller `node_loss` is always better. Equal-loss traces use
ordering only to select one deterministic representative; tie-breakers do not
change objective value.

Because Rust `BinaryHeap` pops the greatest element, `Priority::cmp` reverses
the ascending fields. The effective pop order is:

| Precedence | Preferred value | Role |
|---|---|---|
| 1 | Lower incurred HP loss | Normative objective |
| 2 | Lower heuristic estimate | Ordering only |
| 3 | Higher `metrics.powers_played` | Deterministic tie-breaker |
| 4 | Lower total living enemy HP | Deterministic tie-breaker |
| 5 | Higher player block | Deterministic tie-breaker |
| 6 | Lower depth | Deterministic tie-breaker |
| 7 | Lexicographically lower path key | Stable chosen trace |
| 8 | Lower node insertion index | Total ordering |

`TraceStep` records the canonical child state hash and running HP loss after
each action. `action_ids` is the same trace projected to action ids.

`compare` solves baseline and candidate independently in exact/zero mode. Its
`hp_loss_delta` is `candidate - baseline` only when both won. `better` is
`baseline` or `candidate` when exactly one won, `neither` when neither won,
otherwise the lower-loss side or `tie`. Consumers MUST NOT present `better` as
conclusive when either nested result is incomplete.

### SRCH-002 — Exact traversal, equivalence, and proof

| Rule | Owner / Where | Why |
|---|---|---|
| Exact mode MUST order primarily by incurred objective cost | `Priority` | First win then has minimum loss |
| Every child MUST come from the combat engine | `solve_internal` | Search cannot invent transitions |
| Canonical state id MUST key deduplication | `best_loss` | Rust layout cannot define identity |
| A state is revisited only for strictly lower path loss | child insertion | Equal/worse paths cannot improve result |
| Stale queued entries MUST be skipped | frontier pop | A later better path supersedes them |
| Chosen trace MUST be action-order independent | priority/path key | Enumeration order is not semantics |

Search reduces legal actions to one representative when supported mechanics
make card instances behaviorally identical. The equivalence key includes action
type, card model, upgrade, base/effective cost, retained/exhaust/ethereal flags,
and target. A selection uses the selected hand instance to build the same key.
Instance ids remain observable in the chosen action and snapshots, but swapping
otherwise identical supported cards does not create another search branch.

Deduplication maps the full canonical combat-state hash to its best incurred
loss. A child whose loss is greater than or equal to the recorded loss is a
cache hit and is not queued. On pop, a node whose recorded best loss changed is
also a cache hit. `explored_states` counts expanded nonterminal nodes and losing
terminal nodes; stale entries do not count.

Exact proof rules are:

| Observation | Claim allowed |
|---|---|
| First winning node popped | Win is minimum HP loss because no higher-loss node can precede it |
| Frontier exhausted with no cutoff and no win | No winning line exists in the supported graph |
| Timeout, state cutoff, or reachable turn cutoff | No completeness or optimality claim |

The heuristic is below incurred loss in priority, so admissibility is not needed
for current exactness: it only reorders equal-loss nodes. Moving a heuristic
ahead of objective cost, pruning by it, or weakening representative-action
equivalence requires an accepted spec change and new exhaustive conformance.

### SRCH-003 — Limits, result truth table, and approximate mode

| Limit | Default | Current enforcement |
|---|---:|---|
| `max_states` | 100,000 | Stop before expanding when explored count reached |
| `max_turns` | 50 | Skip nodes whose combat turn is greater; remember cutoff |
| `timeout_seconds` | 60.0 | Check elapsed host/WASM clock at every frontier pop |

A winning node is recognized before state/turn cutoff checks. Losing nodes are
counted and discarded. If any reachable node exceeds the turn limit and no win
is returned, the final result is incomplete `max_turns`; the engine MUST NOT
misreport frontier exhaustion as proof.

`SearchMode::Approximate` currently performs the same traversal, ordering,
deduplication, and limits as `Exact`. It implements no beam, dominance, or other
approximate pruning. After traversal it unconditionally sets
`optimality_proven = false`; all other fields remain those produced by the same
algorithm. This conservative mode exists as a contract boundary for future
explicit pruning, not as a current speed optimization.

| Mode and termination | `won` | `complete` | `optimality_proven` | Optional result fields | Reason |
|---|---:|---:|---:|---|---|
| Exact, first optimal win | true | true | true | HP loss, final HP, nonempty-or-empty trace | `optimal_win` |
| Exact, frontier exhausted | false | true | true | HP/final HP absent; trace empty | `no_winning_line` |
| Exact, any resource cutoff | false | false | false | HP/final HP absent; trace empty | `timeout`, `max_states`, or `max_turns` |
| Approximate, winning traversal | true | true | false | Same winning values and trace | `optimal_win` |
| Approximate, exhausted traversal | false | true | false | HP/final HP absent; trace empty | `no_winning_line` |
| Approximate, resource cutoff | false | false | false | HP/final HP absent; trace empty | Cutoff reason |

`complete` means traversal ended without a resource cutoff; it does not by
itself authorize an optimality claim. `optimality_proven` is the only proof flag.
An incomplete result carries no incumbent, even if future algorithms add one.

**PROHIBITED:**

- claiming that an incomplete result has no winning line;
- setting approximate `optimality_proven` true;
- pruning on a heuristic under the current exact contract;
- using action enumeration or hash-map iteration as a tie-breaker;
- interpreting runtime or explored-state count as a correctness proof.

### SRCH-004 — Heuristic ownership and invariance

`rusty-spire-simulator` owns the `Heuristic` interface and `ZeroHeuristic`.
`rusty-spire-heuristics` depends on that interface and provides
`RemainingEnemyHp`, the sum of `max(enemy.hp, 0)`. The simulator MUST NOT depend
on the heuristics crate, preserving the dependency DAG.

| Heuristic | Estimate | Exact-mode effect |
|---|---|---|
| `ZeroHeuristic` | Always 0 | Baseline deterministic ordering |
| `RemainingEnemyHp` | Sum of living enemy HP | Orders equal-loss nodes toward lower enemy HP |

For identical input and sufficient limits, exact search with either implementation
MUST return the same completed objective value and proof status. A heuristic MAY
change the chosen equal-cost trace because it precedes `path_key` in priority, and it
MAY change exploration order and resource usage. Reversing legal-action enumeration
with the same heuristic MUST retain the deterministic trace covered by SRCH-002.

### Non-goals

- No probabilistic expectimax, Monte Carlo sampling, or hidden-RNG branching.
- No Pareto or multi-objective result beyond `MinimizeHpLoss`.
- No current approximate-pruning algorithm or performance guarantee.
- No proof outside SPEC-005's supported deterministic combat graph.

## Conformance

| Requirement | Automated evidence | Required assertions |
|---|---|---|
| SRCH-001 | `test:exact_search`; `converted_silent_weak_seed_matrix_keeps_equivalent_optima` | Known exhaustive fixtures return the promoted minimum HP loss and final HP |
| SRCH-002 | `test:exact_search`; `optimal_trace_is_independent_of_action_enumeration_order` | Canonical dedup and deterministic priority produce the same exact trace under reversed enumeration |
| SRCH-003 | `test:search_modes`; `approximate_mode_never_claims_optimality` | Approximate proof is always suppressed and cutoff results remain incomplete |
| SRCH-004 | `test:heuristics`; `exact_results_are_heuristic_independent`; `terminal_state_has_zero_remaining_hp` | Zero and remaining-HP ordering preserve completed exact objective and proof |

An optimality assertion MUST check `optimality_proven`, not infer proof from
`won`, `complete`, termination text, or a matching fixture value alone.

## References

- [SPEC-003: Combat Domain and State Invariants](003-domain.md)
- [SPEC-005: Combat Initialization and Transition Semantics](005-combat.md)
- [Search implementation](../crates/rusty-spire-simulator/src/search.rs)
- [Objective implementation](../crates/rusty-spire-simulator/src/objective.rs)
- [Heuristic implementations](../crates/rusty-spire-heuristics/src/lib.rs)
- [Silent weak evidence matrix](../fixtures/evidence/silent_weak_seed_matrix.json)
- [Traceability manifest](traceability.json)
