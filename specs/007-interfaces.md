---
id: SPEC-007
title: Versioned Application and Wire Interfaces
status: accepted
domain: interfaces
version: 1
applies_to: v0.3
depends: [SPEC-001, SPEC-002, SPEC-003, SPEC-005, SPEC-006]
sources: [crates/rusty-spire-api/src/lib.rs, crates/rusty-spire-wasm/src/lib.rs, apps/cli/src/main.rs, apps/web/src/simulator-worker.ts, specs/schemas]
---

# SPEC-007: Versioned Application and Wire Interfaces

## Status

ACCEPTED

## Summary

Rusty Spire v0.3 exposes one package-bound application service through Rust, JSON, CLI, and WebAssembly while preserving explicitly deprecated v0.2 CLI and browser contracts until v0.4.

## Specification

### Contract layers

| Layer | Current authority | Stability in v0.3 |
|---|---|---|
| Rust domain and engine | Owning crate types and methods | Source compatibility not promised |
| Rust application service | `rusty-spire-api::AppService` | Shared behavior authority |
| JSON operations | `ApiOperationV1` Serde representation | Versioned wire contract |
| WASM memory ABI | `rusty-spire-wasm` exports | Versioned dispatcher plus legacy export |
| Generated JSON Schemas | `specs/schemas/` | Partial structural validation described below |
| Generated TypeScript | `apps/web/src/contracts.generated.ts` | Browser-consumer subset, not full Rust API |

The implementation still contains transitional aliases:

```rust
// crates/rusty-spire-data/src/catalog.rs
pub type DataPackage = CombatCatalog;

// crates/rusty-spire-combat/src/engine.rs
pub type CombatEngine<'a> = Simulator<'a>;
```

`DataPackageV1` is the committed grouped wire format, but `CombatCatalog::from_json`
currently normalizes that format into the legacy catalog representation. Likewise,
`CombatEngine`, `CombatError`, and their `Simulator` names are the same Rust types in
v0.3. They are not yet independent implementations or compatibility facades.

### API-001 — Version requests and separate combat from search

| Contract | Required fields and behavior | Current limitation |
|---|---|---|
| `CombatSetupV2` | Version 2, `{package_id, sha256}`, RNG, character, deck, relics, empty potions, encounter | Internally adapts to `CombatSetupV1` before initialization |
| `SolveRequestV1` | Version 1, setup, policy, mode, heuristic, limits | Only `minimize_hp_loss` is accepted |
| `CompareRequestV1` | Version 1, baseline, candidate, policy, limits | No mode/heuristic fields; current comparison uses exact zero-heuristic solve |
| `PackageIdentityV1` | Non-empty package ID and exact package SHA-256 | Identity must match the loaded package |

Input DTOs carrying `deny_unknown_fields` MUST reject unknown top-level fields.
Version values MUST be validated before behavior is executed. Package mismatch MUST
fail before combat initialization. `CombatSetupV2` MUST contain combat inputs only;
search choices belong to solve or compare requests.

Defaults are part of the v1 request contract: exact search, zero heuristic,
`minimize_hp_loss`, and `SolveLimits::default()`. A caller that relies on a default
still receives the same completeness and proof semantics defined by SPEC-006.

### API-002 — Expose tagged outputs and stable service errors

| DTO | Current wire shape |
|---|---|
| `CombatActionV1` | `type`-tagged `card`, `end_turn`, or `choose` variant |
| `CombatSnapshotV3` | Version 3 wrapper around the current `CombatState` |
| `SolveResponseV1` | Version, `SolveResult`, opening hand IDs, tagged actions |
| `CompareResponseV1` | Version and `CompareResult` |
| `ContentManifestV1` | Package, game version, characters, cards, enemies, relics |
| `ApiErrorV1` | Version 1, stable code, human message |

The stable error codes are:

```text
invalid_json, invalid_request, package_mismatch, unknown_id,
unsupported, invalid_action, internal
```

Application-service and `sls2_call_v1` failures MUST use `ApiErrorV1`. Messages MAY
gain detail, but consumers MUST branch only on the code. `CombatSnapshotV3::state_id`
MUST reject wrapper versions other than 3 and derive identity using the canonical
state rule in SPEC-003.

The committed generator currently emits five schemas: `CombatSetupV2`,
`SolveRequestV1`, `ApiErrorV1`, `ContentManifestV1`, and `CombatSnapshotV3`.
Nested setup objects, manifest array items, solve limits, and snapshot state are only
shallowly constrained; there are no generated schemas yet for compare, actions, or
responses. Runtime Rust DTO validation remains authoritative for those details.
Tests and documentation MUST NOT claim full JSON Schema coverage until the generator
actually emits complete shapes.

### API-003 — Route clients through one service and dispatcher

`AppService` owns the selected `DataPackage` handle and exposes content, validation,
solve, compare, and legacy adapter methods. JSON callers use these operations:

| Operation tag | Input | Success value |
|---|---|---|
| `content_info` | No payload | `ContentManifestV1` |
| `validate` | `CombatSetupV2` | `{valid, setup_hash}` |
| `solve` | `SolveRequestV1` | `SolveResponseV1` |
| `compare` | `CompareRequestV1` | `CompareResponseV1` |

`AppService::call_json` MUST return one JSON envelope:

```json
{"ok": true, "value": {}}
```

or:

```json
{"ok": false, "error": {"schema_version": 1, "code": "invalid_request", "message": "..."}}
```

The `rusty-spire api --input ... [--output ...]` command and `sls2_call_v1`
MUST pass the operation bytes to this service rather than reimplementing dispatch.

The WASM ABI exports `sls2_alloc`, `sls2_free`, and
`sls2_call_v1(pointer, length) -> u64`. The returned `u64` packs the output pointer in
the low 32 bits and output length in the high 32 bits. A host MUST free both its input
allocation and the returned output allocation with their exact lengths. A null input
pointer MUST produce a structured `invalid_request` envelope rather than trap.

### API-004 — Bound the v0.2 compatibility window

Version 0.3 MUST continue to accept:

- `CombatSetupV1` through the legacy application methods;
- `solve`, `compare`, `validate`, and `catalog-info` CLI commands and flags;
- legacy result, trace, and action serialization from those CLI commands; and
- `sls2_solve_json(pointer, length, max_states, max_turns, timeout_millis)`.

The exact v0.2 catalog hash is translated to the embedded v0.3 package hash before
legacy validation. Legacy input MUST select legacy output automatically. The legacy
CLI commands MUST print a deprecation notice to stderr; the versioned `api` command
MUST NOT. The legacy WASM export retains its string-valued error envelope and is not
an `ApiErrorV1` surface.

These adapters are scheduled for removal in v0.4. Removal requires an accepted spec
amendment, fixture updates, and a major wire migration note. Compatibility does not
cover the old monolithic Rust source API or the alias names shown above.

### API-005 — Drive browser setup from embedded content

The Web application MUST obtain package identity, character records, starter decks,
enemy HP ranges, supported content labels, and assets from `ContentManifestV1`
through `sls2_call_v1`. TypeScript MUST NOT contain a copied package hash, catalog
record, or combat transition. Browser solve requests MUST include the manifest
identity unchanged.

`ContentManifestV1` does not yet expose an RNG-profile default, ascension thresholds,
maximum ascension, or preferred initial character/enemy. The v0.3 page therefore
contains protocol/UI defaults for those values (`isolated_combat_xoshiro_v1`, 8, 10,
Silent, and Nibbit) and resolves the selected IDs against the returned manifest before
building a request. Rust validation remains authoritative. Moving these defaults into
the manifest is future contract work, not a current content-authority claim.

The generated TypeScript file currently models only the Web consumer subset and
retains legacy `SolveResult`/action types; it is not a complete projection of every
Rust v1 DTO. The worker currently converts `ApiErrorV1` to `Error(message)` and sends
only that string to the React thread. Therefore structured error-code stability ends
at the worker boundary in v0.3; Web code MUST NOT infer a machine code from message
text. Preserving `{code, message}` through the worker requires a future API amendment
and matching tests.

## Conformance

| Requirement | Automated evidence | Review evidence |
|---|---|---|
| API-001 | `test:api_v1`, `test:unsupported_content` | V2 adaptation and compare limitations match code |
| API-002 | `test:api_errors`, `check:schemas`, `test:canonical_state_id` | Schema coverage is not overstated |
| API-003 | `test:shared_service`, `test:api_v1` | CLI and WASM both delegate to the service dispatcher |
| API-004 | `test:cli_legacy`, `test:wasm_legacy` | Deprecation and output selection remain intact |
| API-005 | `test:web_content_manifest`, `test:web` | No copied identity; error-code loss remains explicit |

## References

- [SPEC-003: Combat Domain and State Invariants](003-domain.md)
- [SPEC-005: Combat Initialization and Transition Semantics](005-combat.md)
- [SPEC-006: Exact Search and Proof Semantics](006-search.md)
- [Generated schemas](schemas/)
- [API implementation](../crates/rusty-spire-api/src/lib.rs)
- [WASM implementation](../crates/rusty-spire-wasm/src/lib.rs)
- [Web worker](../apps/web/src/simulator-worker.ts)
