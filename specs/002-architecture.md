---
id: SPEC-002
title: Architecture and Crate Boundaries
status: accepted
domain: architecture
version: 2
applies_to: v0.3 active Rust workspace and application boundaries
depends: [SPEC-001]
sources: [Cargo.toml, tools/specs/check_architecture.py, crates/rusty-spire-core/src/lib.rs, crates/rusty-spire-data/src/lib.rs, crates/rusty-spire-combat/src/lib.rs, crates/rusty-spire-simulator/src/lib.rs, crates/rusty-spire-heuristics/src/lib.rs, crates/rusty-spire-api/src/lib.rs, crates/rusty-spire-wasm/src/lib.rs, apps/cli/src/main.rs, apps/web/src/simulator-worker.ts]
---

# SPEC-002: Architecture and Crate Boundaries

## Status

ACCEPTED

## Summary

Rusty Spire separates state, reviewed data, deterministic combat, graph search,
application contracts, and platform adapters through a one-way dependency
graph that is checked from the active workspace manifests.

## Specification

The active architecture covers the eight packages listed in the root
`Cargo.toml`, the static Web application, and the Spire Codex tools. The
archived bridge is historical material, not an active component. In the
diagrams below, `A -> B` means “A depends on B.”

### ARC-001 — Dependency direction

Every active local Rust dependency **MUST** be an edge allowed by this graph,
and the workspace **MUST NOT** contain an unclassified active package.

```text
apps/cli -----------------------> api <----------------------- wasm <- Web
                                  |  \
                                  |   +---------------------> heuristics
                                  |                            |
                                  +--------------------------> simulator
                                                                |
simulator ----------------------------------------------------> combat
                                                                |
combat -------------------------------------------------------> data
                                                                |
data ---------------------------------------------------------> core

api also has direct edges to core, data, combat, and simulator.
simulator has direct edges to core and data in addition to combat.
heuristics has a direct edge to core in addition to simulator.
```

The normative allowed-local-dependency matrix is the `ALLOWED` value in
`tools/specs/check_architecture.py`:

| Active package | Permitted local dependencies | Forbidden examples |
|---|---|---|
| `rusty-spire-core` | none | data, combat, search, API, platforms |
| `rusty-spire-data` | core | combat or application services |
| `rusty-spire-combat` | core, data | simulator, heuristics, API |
| `rusty-spire-simulator` | core, data, combat | heuristics, API, platforms |
| `rusty-spire-heuristics` | core, simulator | combat implementation access not exposed by simulator |
| `rusty-spire-api` | core, data, combat, simulator, heuristics | CLI, WASM, Web |
| `rusty-spire-cli` | API | direct core/data/combat/search dependencies |
| `rusty-spire-wasm` | API | direct engine or data dependencies |

The simulator **MUST NOT** depend on `rusty-spire-heuristics`; heuristics
implement the simulator-owned `Heuristic` interface, which prevents a cycle.
The checker evaluates path dependencies reported by `cargo metadata`; external
crate selection and feature policy are outside this requirement.

### ARC-002 — Core boundary

`rusty-spire-core` **MUST** remain the platform-independent owner of the combat
domain vocabulary and deterministic primitives, with no local workspace
dependency.

| Core-owned surface | Current source | Boundary |
|---|---|---|
| `CombatState`, `Action`, `Decision`, actor/card/power state | `crates/rusty-spire-core/src/state.rs` | Data-shaped state without catalog lookups |
| `ModelId` and its namespace syntax | `crates/rusty-spire-core/src/id.rs` | Generic identity validation |
| Xoshiro state reconstruction and domain seeds | `crates/rusty-spire-core/src/rng.rs` | Deterministic math without combat policy |
| Canonical JSON and BLAKE3 identifiers | `crates/rusty-spire-core/src/canonical.rs` | Serialization primitives, not versioned application DTOs |

These are actual public signatures from the core crate:

```rust
pub fn state_id<T: Serialize>(value: &T) -> Result<String, CanonicalError>;
pub fn combat_state_id(state: &CombatState) -> Result<String, CanonicalError>;
pub fn next_int(
    algorithm: &str,
    stream: &mut RngStreamState,
    max_exclusive: u32,
) -> u32;
```

Core **MAY** use Serde and JSON values to implement its stable serialization
primitives. It **MUST NOT** own content packages, package loading, filesystem or
network access, clocks, graph search, CLI behavior, Web/WASM ABI code, or the
versioned wire DTOs owned by `rusty-spire-api`. Validation that requires static
content belongs downstream in data or combat.

### ARC-003 — Data and combat boundaries

The data, combat, simulator, and heuristics crates **MUST** each own one stage
of the runtime pipeline and **MUST NOT** absorb a downstream stage.

```text
package bytes
    -> rusty-spire-data
    -> rusty-spire-combat
    -> rusty-spire-simulator
    <- rusty-spire-heuristics
```

| Crate | Required ownership | Prohibited ownership |
|---|---|---|
| data | Package/catalog deserialization, static definitions, closed effect declarations, structural validation, raw-byte SHA-256 | State transitions, search, files, network |
| combat | Setup validation, initialization, legal actions, immutable steps, terminal rules, engine state IDs | Frontier policy, objectives, time limits, platform DTOs |
| simulator | Objective and heuristic traits, exact graph traversal, deduplication, limits, traces, proof/comparison results | Card/enemy semantics, concrete heuristic implementations, platform code |
| heuristics | Concrete implementations of simulator-owned `Heuristic` | Search orchestration, combat mutation, API selection |

Version 0.3 retains two source-level compatibility aliases; specifications and
agents **MUST** describe them as aliases rather than independent types:

```rust
// crates/rusty-spire-data/src/catalog.rs
pub type DataPackage = CombatCatalog;

// crates/rusty-spire-combat/src/engine.rs
pub type CombatEngine<'a> = Simulator<'a>;
```

The combat entry points on that alias are:

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

Search accepts interfaces rather than the heuristics crate:

```rust
pub fn solve_with(
    catalog: &CombatCatalog,
    combat: &InitializedCombat,
    limits: SolveLimits,
    objective: &dyn CombatObjective,
    heuristic: &dyn Heuristic,
    mode: SearchMode,
) -> Result<SolveResult, SimulatorError>;
```

### ARC-004 — Application boundaries

All platform-facing products **MUST** orchestrate `rusty-spire-api` or its WASM
adapter and **MUST NOT** reimplement content or combat/search semantics.

| Component | Current responsibility | Must not contain |
|---|---|---|
| `rusty-spire-api` | `AppService`, v0.3 DTOs, error mapping, embedded package, v0.2 adapters | Files, CLI rendering, browser memory ABI |
| `apps/cli` | Argument parsing, file I/O, stdout/stderr, exit status | Direct engine rules or catalog interpretation |
| `rusty-spire-wasm` | Allocation, clock import, `sls2_call_v1`, legacy ABI | Alternate service or combat implementation |
| `apps/web` | Static UI, Worker lifecycle, request presentation | Copied package hashes/catalog records or combat transitions |
| `tools/spire-codex` | Network fetch, reviewed promotion, offline verification | Runtime dependency or automatic semantic promotion |
| `archive/sts2-bridge` | Historical evidence only | Workspace membership, CI compilation or execution |

`AppService` is the shared application boundary:

```rust
pub struct AppService {
    package: DataPackage,
}

pub fn call_json(&self, input: &[u8]) -> Vec<u8>;
```

The Web application calls `sls2_call_v1` inside
`apps/web/src/simulator-worker.ts`; it does not import Rust crates directly.
`tools/spire-codex/fetch.py` is the only public network command and delegates HTTP
work to `sync.py`; no other product exposes fetching. Runtime and CI package
verification remain offline. This architecture does not require
the archive to conform to current crate boundaries.

## Conformance

| Requirement | Observable acceptance criterion | Registered verification |
|---|---|---|
| ARC-001 | `cargo metadata` reports exactly the active packages and no forbidden local edge | `check:architecture` |
| ARC-002 | Core has no local dependency and its unit tests cover IDs, RNG, and canonicalization | `check:architecture`, `test:core` |
| ARC-003 | Data/combat dependencies follow the matrix and combat tests exercise package-backed transitions | `check:architecture`, `test:combat` |
| ARC-004 | CLI/WASM depend on API only, Web uses the Worker/WASM path, and tools/archive are not runtime packages | `check:architecture`, `test:web` |

## References

- [SPEC-001: Project Scope and Correctness Policy](001-project.md)
- [SPEC-003: Combat Domain and State Invariants](003-domain.md)
- [SPEC-004: Spire Codex Evidence and Data Packages](004-data.md)
- [SPEC-005: Combat Initialization and Transition Semantics](005-combat.md)
- [SPEC-006: Exact Search and Proof Semantics](006-search.md)
- [SPEC-007: Versioned Application and Wire Interfaces](007-interfaces.md)
- [Workspace manifest](../Cargo.toml)
- [Architecture checker](../tools/specs/check_architecture.py)
