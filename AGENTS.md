# AGENTS.md

**This file routes project context and repository-specific agent operations.** Rusty
Spire's binding product design and behavior rules live in `specs/`. Use the links
below to load the specifications relevant to a task; do not restate those rules here.

Rusty Spire is a spec-driven Rust workspace for deterministic, offline Slay
the Spire 2 combat simulation and search. The exact product scope is defined
by [SPEC-001](specs/001-project.md).

## Start here

- [Specification index](specs/README.md) — every active specification, its
  status, dependencies, requirement range, and context size.
- [SPEC-000](specs/000-specification-guidelines.md) — specification format,
  status lifecycle, normative language, traceability, and amendment process.
- [Project README](README.md) — workspace layout, setup, and common commands.

## Repository exploration

This repository is indexed when `.codegraph/` exists. In that case, use the
CodeGraph MCP `codegraph_explore` operation before grep, file discovery, or
manual code reading; it returns current source plus callers and dependency
paths. If the MCP is unavailable, use `codegraph explore "<question>"` from
the shell. Do not create or rebuild the index unless a maintainer asks.

## Task routing

| If you are changing or investigating... | Read these specifications |
|---|---|
| Product purpose, supported features, exclusions, terminology | [SPEC-001](specs/001-project.md) |
| Crate ownership, dependencies, new crates, or moved types | [SPEC-002](specs/002-architecture.md) |
| Combat state, decisions, identity, branching, or RNG | [SPEC-003](specs/003-domain.md) |
| Spire Codex ingestion, evidence, packages, hashing, or promoted content | [SPEC-004](specs/004-data.md) |
| Setup, legal actions, effect execution, turns, enemies, powers, or relics | [SPEC-005](specs/005-combat.md) |
| Search, objectives, limits, deduplication, heuristics, or proof claims | [SPEC-006](specs/006-search.md) |
| JSON DTOs, schemas, stable errors, CLI/WASM compatibility, or worker ABI | [SPEC-007](specs/007-interfaces.md) |
| CLI, Web, worker, assets, or Spire Codex tool responsibilities | [SPEC-008](specs/008-products.md) |
| Archived bridge evidence or a proposal to reactivate the bridge | [SPEC-009](specs/009-bridge-draft.md) and its accepted dependencies |
| CI topology, required checks, coverage, generated artifacts, or release gates | [SPEC-010](specs/010-conformance.md) |
| Writing, splitting, accepting, retiring, or amending a specification | [SPEC-000](specs/000-specification-guidelines.md) |

## Supporting indexes

| Topic | Pointer |
|---|---|
| Requirement-to-check mappings | [specs/traceability.json](specs/traceability.json) |
| Registered automated checks | [specs/checks.json](specs/checks.json) |
| Public wire schemas | [specs/schemas/](specs/schemas/) |
| Implemented and published content | [specs/content/](specs/content/) |
| Data provenance and promotion | [docs/data_sources.md](docs/data_sources.md) |
| Simulator usage and explanation | [docs/simulator.md](docs/simulator.md) |
| Web application development | [apps/web/README.md](apps/web/README.md) |
| Spire Codex tooling | [tools/spire-codex/README.md](tools/spire-codex/README.md) |

Status meaning, precedence, and the process for missing or conflicting rules
are defined only by [SPEC-000](specs/000-specification-guidelines.md).
