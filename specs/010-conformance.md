---
id: SPEC-010
title: Conformance and CI Policy
status: accepted
domain: quality
version: 1
applies_to: v0.3
depends: [SPEC-000, SPEC-001, SPEC-002, SPEC-004, SPEC-007, SPEC-008]
sources: [.github/workflows/ci.yml, specs/traceability.json, specs/checks.json, tools/specs/check.py, Cargo.toml, apps/web/package.json]
---

# SPEC-010: Conformance and CI Policy

## Status

ACCEPTED

## Summary

Every accepted Rusty Spire requirement must map to reproducible checks, and CI must gate governance, native Rust, Python tooling, Web/WASM, release builds, and line coverage without a performance benchmark in v0.3.

## Specification

### CI topology

The workflow starts with governance. Rust, Python, Web/WASM, build, and coverage jobs
must not bypass a failed governance job.

```text
governance
   ├── Rust formatting, lint, and tests
   ├── Python tool tests
   ├── Web and WASM tests
   ├── release workspace + static Web build
   └── native Rust coverage
```

Dependency downloads and action checkout use normal CI network access. Gameplay data
checks use committed evidence and do not contact Spire Codex or another runtime data
service; CI-001 and SPEC-004 bind that behavior.

### CI-001 — Gate accepted claims through governance

Before language jobs, CI MUST run:

| Check | Required command | Failure meaning |
|---|---|---|
| Spec format and traceability | `python3 tools/specs/check.py` | Invalid or unverified requirement set |
| Generated contracts | `python3 tools/specs/generate_contracts.py --check` | Schema/TypeScript drift |
| Data package | `python3 tools/spire-codex/verify.py` | Non-reproducible promoted package |
| Content status | `python3 tools/specs/generate_content_status.py --check` | Implementation/publication drift |
| Crate boundaries | `python3 tools/specs/check_architecture.py` | Forbidden dependency edge |

Every accepted requirement MUST map through `traceability.json` to one or more IDs in
`checks.json`. Registered checks MUST name an executable command and existing source
files. Draft and retired requirements MUST NOT be mapped. Broken links, unknown
dependencies, cycles, stale index entries, duplicate IDs, or active-spec budget
violations MUST fail governance.

### CI-002 — Enforce Rust formatting lint and tests

The Rust job MUST use the pinned toolchain from the workflow and run:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo clippy --target wasm32-unknown-unknown -p rusty-spire-wasm --locked -- -D warnings
cargo test --workspace --all-features --locked --no-fail-fast
```

Lockfile use is mandatory. Native and `wasm32-unknown-unknown` linting are separate
because platform code differs. Archived bridge code is outside the Rust workspace and
MUST NOT be compiled or executed by this job.

### CI-003 — Require Python Web WASM and host checks

| Surface | Mandatory evidence |
|---|---|
| Spire Codex tools | Standard-library unit tests under `tools/spire-codex/tests` |
| Specification tools | Unit tests under `tools/specs/tests` |
| Web and worker | `npm test` from `apps/web` after `npm ci` |
| WASM | Browser-host tests exercising versioned and legacy exports |
| Host/service routing | `AppService` Rust tests plus CLI and WASM host tests |

Rust service and CLI tests MUST exercise their implemented success, incomplete, and
failure surfaces. CLI and WASM both delegate versioned operations to `AppService`, but
v0.3 has no single cross-host fixture asserting byte-for-byte parity; CI MUST NOT
claim that stronger guarantee. Current Web tests cover the static build, committed
WASM legacy solve, versioned `content_info`, and content-driven identity/assets. They
do not directly instantiate the Worker or assert versioned solve/compare/validate,
null-pointer errors, or incomplete/error UI presentation; those remain explicit test
gaps rather than accepted coverage claims.

### CI-004 — Verify reproducible release artifacts offline from source data

The build job MUST compile the locked release workspace, smoke-test
`rusty-spire --version`, verify the committed WASM source fingerprint, smoke-test the
committed module, and produce the static Web bundle. The bundle MUST contain HTML,
JavaScript, CSS, and WASM output.

Generated schemas, TypeScript contracts, the promoted package, and the WASM source
fingerprint MUST have deterministic check modes. A check MUST fail if regeneration
would change a committed artifact. Spire Codex network fetches are prohibited; CI
reconstructs only from committed evidence, reviewed inputs, and the base catalog
documented by SPEC-004/008.

The committed `.wasm` binary is smoke-tested, while its source fingerprint proves
that relevant Rust/package inputs have not changed without regeneration. Reproducible
byte-for-byte WASM compilation across toolchains is not currently claimed.

### CI-005 — Maintain the native Rust line coverage floor

CI MUST enforce at least 85% aggregate line coverage over these active native
packages:

```text
rusty-spire-core, rusty-spire-data, rusty-spire-combat,
rusty-spire-simulator, rusty-spire-heuristics, rusty-spire-api,
rusty-spire-cli
```

Coverage is aggregate, not a promise that each file or crate individually reaches
85%. `rusty-spire-wasm`, Web TypeScript, Python, generated files, and archived bridge
code are not part of this numeric Rust gate; their mandatory functional tests remain
covered by CI-003. Lowering the threshold or excluding another active native crate
requires an accepted amendment with review rationale.

### CI-006 — Defer performance gates until workloads are representative

Version 0.3 MUST NOT contain a merge-blocking state-count/time benchmark. Wall-clock
time varies by runner, and the current content slice is too small to define durable
performance expectations. Functional search limits remain correctness-visible under
SPEC-006, but they are not throughput targets.

A future performance gate requires an accepted amendment defining:

1. representative versioned scenarios and package identities;
2. warm-up, repetitions, runner class, and timing method;
3. state-count, memory, latency, and variance metrics as appropriate;
4. baseline collection and statistically justified regression thresholds;
5. exact versus approximate mode separation; and
6. an explicit process for intentional baseline changes.

Microbenchmarks MAY exist for local investigation, but MUST NOT make correctness or
optimality claims and MUST NOT block v0.3 merges.

## Conformance

| Requirement | Automated evidence | Review evidence |
|---|---|---|
| CI-001 | `check:specifications`, `check:schemas`, `check:data_verify`, `check:content_status`, `check:architecture` | Governance is prerequisite for language jobs |
| CI-002 | `check:rust_quality`, `test:workspace` | Locked native and WASM commands match workflow |
| CI-003 | `test:spire_codex_tools`, `test:spec_tools`, `test:web`, `test:shared_service` | All active language surfaces remain mandatory |
| CI-004 | `check:release_artifacts`, `check:schemas`, `check:data_verify`, `check:wasm_fingerprint` | No Spire Codex fetch or byte-reproducible WASM overclaim |
| CI-005 | `check:coverage` | Aggregate scope and 85% threshold match workflow |
| CI-006 | `check:no_performance_gate` | No merge-blocking benchmark job or test |

## References

- [SPEC-000: Specification Format and Governance](000-specification-guidelines.md)
- [SPEC-004: Spire Codex Evidence and Data Packages](004-data.md)
- [SPEC-006: Exact Search and Proof Semantics](006-search.md)
- [SPEC-007: Versioned Application and Wire Interfaces](007-interfaces.md)
- [SPEC-008: CLI Web and Data Tool Responsibilities](008-products.md)
- [CI workflow](../.github/workflows/ci.yml)
- [Traceability manifest](traceability.json)
- [Registered checks](checks.json)
