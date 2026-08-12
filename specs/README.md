# Rusty Spire specifications

This directory is the normative source for Rusty Spire v0.3 behavior and
architecture. Each specification has stable metadata and requirement IDs.
`status: accepted` requirements are binding and must be mapped in
`traceability.json`; draft specifications are informative.

Run the governance checks with:

```bash
python3 tools/specs/check.py
python3 tools/specs/check_architecture.py
```

Documentation under `docs/` explains the system but cannot override these
specifications.
