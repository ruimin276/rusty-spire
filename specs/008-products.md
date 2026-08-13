---
id: SPEC-008
title: CLI Web and Data Tool Responsibilities
status: accepted
domain: applications
version: 2
applies_to: v0.3
depends: [SPEC-001, SPEC-002, SPEC-004, SPEC-007]
sources: [apps/cli/src/main.rs, apps/web/app/combat-replay.tsx, apps/web/src/simulator.ts, apps/web/src/simulator-worker.ts, tools/spire-codex/fetch.py, tools/spire-codex/promote.py, tools/spire-codex/verify.py]
---

# SPEC-008: CLI Web and Data Tool Responsibilities

## Status

ACCEPTED

## Summary

Rusty Spire's CLI, static Web application, and Spire Codex tools own platform I/O and presentation while all combat, package validation, and search semantics remain in shared Rust libraries.

## Specification

### Product boundary

| Product | Owns | Must delegate |
|---|---|---|
| `apps/cli` | Arguments, files, stdout/stderr, exit status | Validation, combat, solve, compare |
| `apps/web` | React UI, worker lifecycle, WASM memory transport | Content authority, combat, search |
| `tools/spire-codex` | Network ingestion, promotion, offline verification | Runtime package validation |

Application code does not interpret card descriptions, duplicate effect execution,
or provide a second search algorithm. Platform defaults are bounded by SPEC-007.

### APP-001 — Keep the CLI a file and process adapter

The binary name is `rusty-spire`. Its current commands are:

| Command | Contract | Output and failure behavior |
|---|---|---|
| `api` | `--input`, optional `--output`; embedded package | Writes the v1 `{ok,value|error}` envelope |
| `solve` | V1 setup, catalog, limits, optional policy/debug flag | Legacy `SolveResult`; warns on stderr |
| `compare` | Baseline/candidate V1 setups, catalog, limits | Legacy `CompareResult`; warns on stderr |
| `validate` | V1 setup and catalog | Validation summary on stdout; warns on stderr |
| `catalog-info` | Catalog path | Source, hashes, profiles, ascensions, counts; warns |

Legacy command defaults are 100,000 states, 50 turns, and 60 seconds. The CLI MUST
load caller-named files, invoke `AppService`, pretty-print JSON, and return a nonzero
exit status when file parsing or the invoked legacy service method returns an error.
It MUST NOT contain card, enemy, RNG, transition, objective, or proof logic.

The versioned `api` command parses the service envelope and pretty-prints a
semantically equivalent JSON value. A logical
`{"ok":false}` envelope is valid command output and currently does not itself make
the process fail; automation using this command MUST inspect `ok`. This differs from
legacy subcommands, whose Rust errors produce stderr plus failure exit status.

### APP-002 — Keep the Web application static and local-first

`apps/web` is a React application compiled to static files. It MUST run solve work in
a browser Web Worker using the committed Rust WASM module. The main thread may build
requests and render results but MUST NOT execute or approximate combat mechanics.

| Concern | Current owner and behavior |
|---|---|
| Package/content discovery | `content_info` through `sls2_call_v1` |
| Setup construction | React state using manifest content plus documented v0.3 UI defaults |
| Search | Worker calls WASM exact solve with explicit limits |
| WASM lifetime | Worker lazily instantiates and caches one module instance |
| Linear memory | Worker allocates, copies, calls, decodes, and frees both buffers |
| Incomplete results | UI preserves completeness and proof flags |
| Winning replay | Rust/WASM supplies frames and intent; React owns playback and inspection only |
| Production hosting | Any static host; paths remain relative |

The browser may fetch its own same-origin WASM and static assets. It MUST NOT upload
combat inputs/results, call Spire Codex, require authentication, or depend on an
application server. A production build MUST contain the HTML, JS, CSS, and WASM
artifacts and pass the committed WASM source-fingerprint check.

The worker's current error reply is a string. It discards `ApiErrorV1.code` after
extracting the message, and `simulator.ts` rejects with a plain `Error`. UI behavior
MUST treat that value as human-readable only and MUST NOT parse it for control flow.
The page also retains protocol/UI constants for the RNG profile, ascension 8/10
boundaries, and initial Silent/Nibbit selection because the current content manifest
does not carry them. Those constants MUST NOT be treated as catalog authority, and
selected IDs must resolve in the returned manifest before a solve request exists.

Successful results MUST keep proof metrics visible and use the versioned replay as
the primary result view. Replay opens paused at frame zero and provides direct
step/turn selection, previous/next, play/pause, speed, keyboard navigation, actor
HP/block/energy/powers, enemy intent, card artwork, and expandable pile/hash/raw
trace details. Card inspection exposes upgrade, effective cost, pile, and instance
identity. Manual navigation pauses playback, reduced motion disables decorative
transitions, and narrow screens retain touch-usable controls. React MAY derive
display deltas between supplied frames but MUST NOT calculate outcomes or intent.

### APP-003 — Separate fetch promotion and offline verification

The public data-tool operations are:

| Operation | Network | Inputs | Output |
|---|---|---|---|
| `fetch.py` | Required | Spire Codex endpoint, stable/beta selection | Immutable provenance-rich snapshot |
| `promote.py` | Forbidden | Compact evidence, reviewed effects/assets, base catalog | Deterministic `DataPackageV1` JSON |
| `verify.py` | Forbidden | Same committed promotion inputs | Byte comparison with committed package |

Fetch MUST default to stable, require explicit `--channel beta` for beta, preserve
endpoint/content hashes, respect retry/rate settings, and write timestamped snapshots
under the ignored upstream-data area. Fetch is a maintainer operation and MUST NOT run
in CI or at application runtime.

Promotion MUST select reviewed records, use the closed effect vocabulary, sort JSON
keys, and never parse prose descriptions into mechanics. In v0.3 it still uses
`catalogs/combat_v0.107.1.json` as the base for retained characters, monsters,
relics, powers, encounters, RNG profiles, ascensions, and modifiers. This is a
documented migration limitation: compact selected evidence is not yet the sole input
for every retained field.

Verification MUST reconstruct package bytes without network access and fail on any
diff. `sync.py` and `build_catalog.py` remain implementation/legacy helpers rather
than additional public lifecycle stages. There is no general network asset-sync
operation in the current public tool contract; selected Web asset paths are reviewed
promotion inputs and their committed files are checked by Web tests.

## Conformance

| Requirement | Automated evidence | Review evidence |
|---|---|---|
| APP-001 | `test:cli`, `test:cli_legacy`, `test:api_v1` | Commands contain platform logic only |
| APP-002 | `test:web`, `test:web_content_manifest`, `test:wasm_legacy`, `test:replay` | Static/local behavior, replay presentation, and error limitation match code |
| APP-003 | `test:spire_codex_tools`, `check:data_verify`, `test:data_evidence` | CI uses verify, never fetch |

## References

- [SPEC-004: Spire Codex Evidence and Data Packages](004-data.md)
- [SPEC-007: Versioned Application and Wire Interfaces](007-interfaces.md)
- [CLI implementation](../apps/cli/src/main.rs)
- [Web application guide](../apps/web/README.md)
- [Web simulator host](../apps/web/src/simulator.ts)
- [Spire Codex tool guide](../tools/spire-codex/README.md)
- [Data source explanation](../docs/data_sources.md)
