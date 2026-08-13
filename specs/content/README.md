# Content status ledgers

These files distinguish engine implementation from package publication:

| Status | Meaning |
|---|---|
| `implemented` | The listed mechanics execute and have registered conformance evidence |
| `recognized_inert` | The model is accepted, but its real effect is outside isolated combat |
| `represented_only` | Data may be published, but combat execution is unsupported |

- `implemented-v1.json` is the reviewed source of truth for which card and
  relic mechanics Rusty Spire realizes, whether or not they are published in
  the current package. It records upgrade coverage, requirement IDs, and
  registered checks. `recognized_inert` means the model is accepted by
  isolated combat but its real game effect belongs to an out-of-scope
  lifecycle.
- `published-v1.json` is generated from the implementation ledger and the
  committed stable data package. It records the exact package identity and
  effect declarations users receive.

Package presence alone does not imply implementation. A content change must
update the reviewed ledger, its conformance tests, and the promoted package as
applicable. Regenerate and verify with:

```bash
python3 tools/specs/generate_content_status.py
python3 tools/specs/generate_content_status.py --check
```

The generator fails when published cards or relics lack an implementation
status, names or declared effects diverge, or referenced requirements/checks
are not registered.
