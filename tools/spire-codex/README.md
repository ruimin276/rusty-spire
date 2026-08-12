# Spire Codex data tools

These scripts use only the Python standard library and are not part of the
simulator runtime. `fetch.py`, `promote.py`, and `verify.py` are the public
ingestion, reviewed-promotion, and offline-verification operations.

- `fetch.py` snapshots the eight combat-related API endpoints. Stable is the
  default; pass `--channel beta` explicitly for beta.
- `build_catalog.py` verifies one immutable snapshot and promotes only IDs from
  a reviewed `CombatCatalogV1` slice.
- `sync.py` contains the crawler implementation.

Run tests from the repository root:

```bash
python3 -m unittest discover -s tools/spire-codex/tests -v
```

See `docs/data_sources.md` for provenance and promotion policy.
