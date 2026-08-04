# Game Data Sources and Snapshots

There is no first-party, structured Mega Crit API for cards, relics, and enemy
mechanics. Official Mega Crit news and Steam announcements are useful freshness
signals, but they are patch notes rather than a machine-readable entity catalog.

## Source policy

Use sources in this order:

1. A locally installed game's assembly and localization files, pinned by build
   id and assembly SHA-256, are the strongest authority for exact simulation.
2. [Spire Codex](https://spire-codex.com/developers) is the default remote feed.
   Its maintainers extract structured data from the game files, including card
   values and enemy move state machines. Its hosted API permits community use
   within published rate limits and exposes stable and beta channels.
3. Community wikis and other databases are cross-checks for descriptions and
   discoverability. They are not sufficient evidence for an exact mechanic.

Do not automate the wiki.gg MediaWiki API. Its current `robots.txt` disallows
`/api.php`. Wiki content can still be consulted manually under its stated
CC BY-SA terms.

The Spire Codex API is still a community service, not an official Mega Crit
database. It gives no dataset-version response header and explicitly makes no
schema, freshness, or correctness guarantee. The crawler therefore records the
latest build-linked changelog as *version evidence*, not as proof that the data
matches the newest Steam build.

## Fetching a snapshot

From an editable install or with `PYTHONPATH=src`:

```bash
.venv/bin/python scripts/fetch_game_data.py
```

By default the command snapshots stable and beta independently. Use
`--channel stable` or `--channel beta` to fetch only one. Requests are
sequential with a 1.05-second delay, staying within the API terms' published
60-request-per-minute limit. It does not fetch artwork. Output is stored under:

```text
data/upstream/spire_codex/<channel>/<game-version>/<UTC-time>-<content-hash>/
├── manifest.json
├── raw/
│   ├── cards.json
│   ├── changelogs.json
│   ├── monsters.json
│   └── relics.json
└── normalized/
    ├── cards.json
    ├── monsters.json
    └── relics.json
```

The beta snapshot also retains `/api/beta/version` and `/api/beta/diff`.
Raw bytes are retained for auditability. Normalized files preserve the source
fields, sort records by source id, and add simulator-style `model_id` values
such as `CARD.NEUTRALIZE` and `MONSTER.NIBBIT`. `manifest.json` records UTC
retrieval time, URLs, response headers, byte and record counts, SHA-256 hashes,
source terms, and the inferred game/build version. Schema drift, empty feeds,
missing required fields, duplicate ids, invalid JSON, and oversized responses
fail the fetch before publishing a snapshot.

Deduplication keys on both upstream content hashes and the normalization
version. Bump the normalizer version whenever normalized output semantics
change; unchanged upstream bytes will then still produce a new auditable
snapshot.

Stable and beta are never merged. The local snapshot directory is ignored by
Git. This avoids accidentally
redistributing Mega Crit's game data and keeps large upstream refreshes out of
code reviews. Identical content reuses the previous snapshot unless `--force`
is passed.

## Simulator promotion rule

Refreshing the catalog must never silently change combat behavior. To promote
a card, relic, enemy, or RNG rule into the Rust simulator:

1. select a specific immutable upstream snapshot;
2. implement the mechanic explicitly;
3. add a golden mechanic test or, preferably, a live trace pinned to the game
   build and assembly hash;
4. review the diff before changing supported-content metadata.

This separates convenient discovery data from the stricter evidence needed to
claim deterministic parity with the game.

For the currently supported combat slice, the reviewed projection and its
verification command are documented in `docs/simulator.md`. The projection is
intentionally small enough to check in; full upstream snapshots remain local
and ignored.

## Terms and attribution

- Spire Codex API terms: <https://github.com/ptrlrd/spire-codex/blob/main/API_TERMS.md>
- Spire Codex developers page: <https://spire-codex.com/developers>
- Mega Crit news: <https://www.megacrit.com/news/>
- wiki.gg robots policy: <https://slaythespire.wiki.gg/robots.txt>

Card, relic, enemy, and other Slay the Spire 2 game data belongs to Mega Crit
Games. Review upstream terms before publishing or redistributing snapshots.
