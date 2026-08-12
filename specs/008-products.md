---
id: SPEC-008
title: Product responsibilities
status: accepted
depends: [SPEC-002, SPEC-007]
---

# Product responsibilities

### APP-001 — CLI

`apps/cli` owns argument parsing, file input/output, process exit status, and
human deprecation notices. It contains no combat or search rules.

### APP-002 — Web

`apps/web` is a portable static React application. Searches run locally in a
Web Worker against the shared WASM engine and upload no combat data.

### APP-003 — Spire Codex tools

`tools/spire-codex` separates `fetch`, `promote`, and offline `verify`
operations. Fetching can never update runtime behavior directly.
