# Spire Codex ingestion and catalog promotion

[Spire Codex](https://spire-codex.com/developers) is the project’s structured
discovery source. It is a community extraction of game data, not an official
Mega Crit API. Runtime simulation never contacts it.

## Immutable snapshots

The standard-library crawler stores cards, characters, relics, potions,
monsters, powers, encounters, and ascensions. Stable is the default; beta must
be selected explicitly and is stored under a separate channel tree.

```bash
python3 tools/spire_codex/fetch.py
python3 tools/spire_codex/fetch.py --channel beta
```

Snapshots are written below
`data/upstream/spire_codex/<channel>/<version>/<timestamp>-<hash>/`. They retain
the exact response bytes, response headers, retrieval time, URLs, endpoint
SHA-256 hashes, aggregate content hash, channel, and reported or inferred game
version. Normalized files preserve source fields, sort by source ID, and add
catalog-style model IDs.

Invalid JSON, oversized or empty feeds, duplicate IDs, required-field drift,
and exhausted retries fail before publication. An unchanged aggregate hash
reuses the prior immutable snapshot unless `--force` is supplied. Full
snapshots are gitignored to avoid redistributing the complete game dataset.

## Reviewed promotion

Fetching cannot modify simulator behavior. Promotion is a separate command:

```bash
python3 tools/spire_codex/build_catalog.py \
  --snapshot data/upstream/spire_codex/stable/<version>/<snapshot> \
  --reviewed catalogs/combat_v0.107.1.json \
  --output /tmp/combat-catalog.json
```

The builder verifies every raw endpoint hash, confirms that each promoted
catalog ID and referenced power exists in that one snapshot, refuses to mix
stable and beta, injects the snapshot provenance, and emits deterministic JSON.
The reviewed file remains the authority for which values and handlers are
promoted. Review the resulting diff, add source-pinned mechanics vectors, then
replace the runtime catalog deliberately and update setup hashes.

Spire Codex parser output alone is insufficient for ordering, targeting,
rounding, RNG consumption, custom commands, and sometimes ascension variants.
For those rules use its decompiled source, parser implementation, and extraction
manifests, plus a pinned executable test, before changing Rust behavior.

## Tests

```bash
python3 -m unittest discover -s tools/spire_codex/tests -v
```

The suite uses local mock servers for stable and beta, retry behavior, schema
drift, duplicate rejection, immutable deduplication, provenance, and
deterministic reviewed-catalog generation.
