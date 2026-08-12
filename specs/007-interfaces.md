---
id: SPEC-007
title: API, CLI, WASM, and Web contracts
status: accepted
depends: [SPEC-002, SPEC-005, SPEC-006]
---

# API, CLI, WASM, and Web contracts

### API-001 — Versioned requests

`CombatSetupV2` contains package identity and combat inputs only. Search
policy, mode, heuristic, and limits belong to versioned solve/compare requests.

### API-002 — Stable responses and errors

Tagged actions, snapshots, solve responses, content manifests, and API errors
are explicitly versioned. Errors expose stable machine codes plus messages.

### API-003 — Shared service

CLI and WASM invoke the same application service. WASM exposes a versioned
operation dispatcher for content-info, validate, solve, and compare.

### API-004 — v0.2 compatibility window

Version 0.3 accepts `CombatSetupV1`, existing CLI commands/flags and
`sls2_solve_json`, returning legacy response shapes for legacy requests. CLI
usage emits a deprecation notice. These adapters are removed in v0.4.

### API-005 — Content-driven Web

The static Web application obtains package identity and supported content from
`ContentManifestV1`; it must not duplicate the catalog hash or combat values.
