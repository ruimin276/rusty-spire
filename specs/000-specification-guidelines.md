---
id: SPEC-000
title: Specification Format and Governance
status: accepted
domain: governance
version: 1
applies_to: repository governance
depends: []
sources: [tools/specs/check.py, specs/traceability.json, specs/checks.json]
---

# SPEC-000: Specification Format and Governance

## Status

ACCEPTED

## Summary

Rusty Spire specifications must be concise, scannable, implementation-accurate
contracts whose stable requirements are linked to reproducible automated
verification.

Requirement index: [source of truth](#gov-001--keep-one-normative-source) ·
[metadata](#gov-002--use-stable-metadata-and-filenames) ·
[status](#gov-003--make-status-match-reality) ·
[sections](#gov-004--follow-the-standard-section-order) ·
[requirements](#gov-005--write-atomic-stable-requirements) ·
[language](#gov-006--use-normative-keywords-and-scannable-rules) ·
[real artifacts](#gov-007--show-real-code-and-artifacts) ·
[verification](#gov-008--link-every-accepted-requirement-to-verification) ·
[references](#gov-009--express-dependencies-and-cross-references-precisely) ·
[budgets](#gov-010--keep-the-active-set-indexable-and-bounded) ·
[amendments](#gov-011--amend-behavior-and-specifications-together) ·
[checklist](#gov-012--complete-the-author-and-reviewer-checklist).

## Scope

This specification governs every normative Markdown file under `specs/`, the
specification index, and the traceability/check registries. It does not define
combat behavior; domain specifications own those rules.

## Specification

### GOV-001 — Keep one normative source

Accepted specifications **MUST** be the source of truth for project behavior and
architecture. Pointer and explanatory documents **MUST** route readers without
creating competing rules.

| Artifact | Role | Authority |
|---|---|---|
| `specs/[0-9][0-9][0-9]-*.md` | Versioned requirements and rationale | Normative when `status: accepted` |
| `specs/schemas/` | Generated public-contract shapes | Normative where an accepted spec delegates to a schema |
| `specs/traceability.json` | Requirement-to-check mapping | Normative traceability record |
| `specs/checks.json` | Check commands and source ownership | Normative verification registry |
| `docs/` | Explanation, tutorials, and background | Non-normative |
| `README.md` and `AGENTS.md` | Navigation, quick start, agent operations | Non-normative for product behavior |

When artifacts disagree, the accepted specification controls. The same change
must repair delegated schemas, tests, generated artifacts, or explanatory text.

### GOV-002 — Use stable metadata and filenames

Every active specification **MUST** begin with this YAML-compatible shape:

```yaml
---
id: SPEC-010
title: Descriptive topic title
status: draft
domain: example
version: 1
applies_to: v0.4 proposal
depends: [SPEC-001, SPEC-002]
sources: [crates/example/src/lib.rs]
---
```

| Field | Required form | Rule |
|---|---|---|
| `id` | `SPEC-NNN` | Globally unique; never reused |
| `title` | Plain text | Specific enough to route without opening the file |
| `status` | `accepted`, `draft`, or `retired` | Meaning is defined by GOV-003 |
| `domain` | Lowercase topic | Used for task routing and index filtering |
| `version` | Positive integer | Increment for material normative amendments |
| `applies_to` | Plain text | Release, component, or governance scope |
| `depends` | Bracketed `SPEC-NNN` list | Direct normative prerequisites only |
| `sources` | Bracketed repository paths | Primary implementation or contract owners |
| Filename | `NNN-short-kebab-title.md` | Numeric prefix must match `id` |

Metadata values remain unquoted and one field per line so the standard-library
checker can parse them without a YAML dependency. Renaming a title may rename
the slug, but neither a rename nor retirement may change the ID.

### GOV-003 — Make status match reality

| Status | Meaning | Requirements bind? | Trace mappings allowed? |
|---|---|---|---|
| `draft` | Proposal, investigation, or unimplemented design | No | No |
| `accepted` | Current implementation and required behavior | Yes | Required for every requirement |
| `retired` | Historical contract replaced or removed | No | No |

The declared status **MUST** match this lifecycle:

```text
draft ──implementation + verification + review──> accepted
  │                                                  │
  ├──rejected──> delete                              ├──replacement/removal──> retired
  └──preserved future work──> remain draft           └──amendment──> accepted revision
```

An accepted specification must not describe aspirational behavior. Future
behavior stays draft until its implementation, conformance tests, traceability
updates, and status transition can land together. Retired specifications remain
root-level and indexed; supporting history may move under `specs/archive/`.

### GOV-004 — Follow the standard section order

Specification sections **MUST** appear in the following order. Conditional
sections are omitted rather than emitted empty.

| Order | Section | Presence | Content |
|---|---|---|---|
| 1 | Frontmatter | Required | ID, title, status, direct dependencies |
| 2 | `# SPEC-NNN: Title` | Required | Exactly matches metadata ID and title |
| 3 | `## Status` | Required | Uppercase value matching metadata status |
| 4 | `## Summary` | Required | One short paragraph stating the contract |
| 5 | `## Scope` | Recommended | Included surfaces and explicit non-goals |
| 6 | `## Terminology` | Conditional | Only terms with project-specific meanings |
| 7 | `## Specification` | Required | Stable H3 requirement entries in ID order |
| 8 | `## Conformance` | Required | Observable acceptance criteria by requirement |
| 9 | `## Compatibility and migration` | Conditional | Wire, data, CLI, ABI, or persistence impact |
| 10 | `## Rationale` | Conditional | Rejected alternatives when the choice is non-obvious |
| 11 | `## References` | Required | Related specs, source, schemas, and evidence |

A long specification should add a linked mini-index after the summary, but
must still respect GOV-010.

### GOV-005 — Write atomic, stable requirements

Each binding rule **MUST** be introduced by an H3 heading in this form:

For example: `### CMB-005 — Resolve block before damage`.

| Rule | Owner / surface | Why |
|---|---|---|
| Use a domain prefix plus three digits | Every requirement | Makes trace failures searchable |
| Keep IDs globally unique | Whole spec set | Prevents ambiguous evidence |
| Assign one observable obligation per ID | Requirement author | Keeps verification meaningful |
| Preserve IDs through amendments | Accepted requirements | Maintains history and fixtures |
| Record removed IDs as tombstones; never renumber or reuse | Changed specifications | Prevents semantic ID reuse |

Prefixes use uppercase ASCII letters or digits, begin with a letter, and name
the domain (`CMB`, `DAT`, `SRCH`, `API`, for example). Headings must use the ID,
an em dash, and an imperative or outcome-oriented title. Prose may explain a
requirement but must not hide additional unnumbered obligations. A removed ID
is recorded in a compatibility tombstone table, not retained as a live H3
requirement, and its traceability mapping is removed.

### GOV-006 — Use normative keywords and scannable rules

The uppercase words **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and
**MAY** carry their RFC 2119 meanings. Lowercase uses are ordinary prose.

| Keyword | Rusty Spire meaning |
|---|---|
| `MUST` / `MUST NOT` | Required for conformance; violations fail review or CI |
| `SHOULD` / `SHOULD NOT` | Default with a documented, reviewed exception |
| `MAY` | Explicitly permitted, never implicitly required |

Multi-rule sections must prefer `Rule | Owner / surface | Why` tables. Short
allow/deny lists are acceptable when ownership is uniform. A table's `Why`
cell should stay under fifteen words; longer rationale belongs below it.
Narrative prose may establish context, but ambiguous phrases such as
“generally,” “where possible,” or “as appropriate” must not define conformance.

### GOV-007 — Show real code and artifacts

Accepted specifications that define code, API, schema, or command behavior **MUST**
quote current relevant names or signatures and identify their source path. For
example, the following is an
actual public function in `crates/rusty-spire-core/src/canonical.rs`:

```rust
pub fn state_id<T: Serialize>(value: &T) -> Result<String, CanonicalError> {
    Ok(blake3::hash(&canonical_json(value)?).to_hex().to_string())
}
```

| Example content | Accepted specification rule |
|---|---|
| Rust types and signatures | Copy current code and name its file |
| JSON requests or snapshots | Validate against the named schema or fixture |
| Dependency diagrams | Match workspace manifests and architecture checks |
| Commands | Use a command registered in `specs/checks.json` |
| Planned API | Keep in a draft spec and label it as proposed |

Idealized pseudocode must not masquerade as implemented API. Ellipses may omit
irrelevant bodies, but not fields, variants, ordering, or errors that affect
the stated contract. If an accepted example and code disagree, update both in
the same change or return the specification to draft.

### GOV-008 — Link every accepted requirement to verification

Each accepted requirement **MUST** have an observable criterion in its
specification's `## Conformance` table and one or more automated check IDs in
`specs/traceability.json`. Every referenced check ID must be registered in
`specs/checks.json` with an executable command and existing source files.

```text
accepted requirement
    └── specs/traceability.json
          └── test:<name> or check:<name>
                └── specs/checks.json
                      ├── command
                      └── source files
```

| Rule | Owner / surface | Why |
|---|---|---|
| Check IDs match `(test|check):[a-z0-9_]+` | Both registries | Stable machine-readable names |
| Criteria describe behavior, not implementation lines | `## Conformance` | Survives refactoring |
| Registries provide requirement-to-test reverse lookup | Check registry | Avoids duplicated annotations |
| Accepted requirements have at least one mapping | Traceability manifest | Prevents unverified rules |
| Draft and retired requirements have no mapping | Traceability manifest | Avoids accidental enforcement |
| Generated output uses a deterministic `--check` mode | Generators | Detects stale artifacts offline |

A broad command such as `cargo test --workspace` is not sufficient evidence by
itself: its registered source list and test names must identify the behavior
that proves the requirement.

### GOV-009 — Express dependencies and cross-references precisely

The `depends` field **MUST** list only specifications whose accepted contracts are
prerequisites for interpreting or implementing the current spec. It must not
be used as a topic tag or a transitive dependency list.

| Reference | Required form |
|---|---|
| Another requirement | `[DOM-003](003-domain.md#dom-003--snapshot-contract)` |
| Another specification | `[SPEC-003](003-domain.md)` |
| Public schema | Relative link under `schemas/` |
| Code or fixture | Repository-relative path in backticks |
| External evidence | Stable URL plus version, revision, or retrieval identity |

Cross-references must say whether the target supplies a prerequisite,
extension, exception, or evidence source. Active specifications may mention a
draft only as explicitly non-binding future work. A cross-reference must not
duplicate the target's rule in paraphrased form.

### GOV-010 — Keep the active set indexable and bounded

The active set **MUST** contain only root-level accepted and draft specifications and
SHOULD
fit in a small number of focused context loads.

| Budget | Preferred | Maximum |
|---|---|---|
| Accepted + draft files | Under 12 | 15 |
| Words per accepted design spec | Under 1,000 | 2,500 |
| Lines per accepted design spec | Under 150 | 350 |
| Requirements per design spec | 4–12 | 20 |

Exceeding a preferred budget requires a clear section index or a split.
Exceeding a maximum requires consolidation, domain splitting, or retirement
before acceptance; lookup data belongs in schemas, packages, fixtures, or
generated references rather than oversized design specs.

`specs/README.md` must list each active spec using these columns:

| Spec | Title | Domain | Status | Requirements | Depends | Lines |
|---|---|---|---|---|---|---|

The index separates accepted and draft specifications, links filenames
relatively, and reports current exact line counts for agent context budgeting.
Retired specifications, when retained, are listed separately and never mixed
with active contracts.

### GOV-011 — Amend behavior and specifications together

Behavior and its accepted specification **MUST** be amended together.

| Change type | Required treatment |
|---|---|
| Editorial correction | Update the spec; verification is unchanged |
| Clarification with no behavior change | Update wording and prove existing tests cover it |
| Behavior or architecture change | Amend accepted requirements and conformance tests in one change |
| New public contract | Start draft; accept with implementation, schema, migration, and tests |
| Requirement removal | Document replacement/migration, retire the ID, and remove its mapping |
| Whole-spec replacement | Link successor, mark old spec retired, and update the index |

An accepted amendment requires maintainer review of both normative text and
evidence. Review must identify compatibility impact, data/package impact,
determinism or RNG impact, and affected interfaces. A temporary mismatch may
exist only within one unmerged change; main must never contain accepted rules
that disagree with implementation.

### GOV-012 — Complete the author and reviewer checklist

Before accepting or materially amending a specification, its author and
reviewer **MUST** confirm:

- [ ] Frontmatter has a stable ID, precise title, valid status, and direct dependencies.
- [ ] Summary stands alone and scope names both included behavior and non-goals.
- [ ] Required sections use the standard order and conditional sections are justified.
- [ ] Every binding obligation has one stable requirement ID and normative wording.
- [ ] Multi-rule sections are tables or short allow/deny lists, not hidden prose.
- [ ] Code, JSON, commands, paths, and diagrams match current repository artifacts.
- [ ] Every accepted requirement has an observable verification criterion.
- [ ] Traceability mappings point to registered checks and existing test sources.
- [ ] Compatibility, migration, deterministic generation, and failure behavior are explicit.
- [ ] Cross-references are relative, live, and do not duplicate another source of truth.
- [ ] The specification and active set remain within GOV-010 budgets.
- [ ] The spec index, schemas, fixtures, generated outputs, tests, and docs are updated together.
- [ ] `python3 tools/specs/check.py` and all mapped commands pass offline.

## Conformance

| Requirement | Automated evidence | Required review evidence |
|---|---|---|
| GOV-001 | `check:specifications` validates the normative index | Confirm explanatory text adds no competing rule |
| GOV-002 | `check:specifications` rejects malformed, duplicate, or mismatched metadata | Confirm title and domain route the topic precisely |
| GOV-003 | `check:specifications` enforces status and accepted-only mappings | Confirm status describes implementation reality |
| GOV-004 | `check:specifications` enforces required section order | Confirm conditional sections are justified |
| GOV-005 | `check:specifications` enforces unique ordered headings and ID grammar | Confirm each requirement is atomic |
| GOV-006 | `check:specifications` enforces numbered uppercase normative rules | Confirm tables and wording are unambiguous |
| GOV-007 | `check:specifications` resolves referenced paths and commands | Compare examples and signatures with current artifacts |
| GOV-008 | `check:specifications` validates conformance/traceability/check agreement | Confirm named tests actually prove the criterion |
| GOV-009 | `check:specifications` resolves links/dependencies and rejects cycles | Confirm references do not duplicate authority |
| GOV-010 | `check:specifications` checks exact index rows and active-set budgets | Confirm preferred-budget exceptions remain readable |
| GOV-011 | `check:specifications` rejects an inconsistent accepted snapshot | Review the pull request for same-change behavior evidence |
| GOV-012 | `check:specifications` covers the mechanically testable checklist subset | Author and reviewer complete the remaining checklist |

## Rationale

This format favors quick retrieval and deterministic checking over long design
essays. It adapts Rumoca's routing-index and compact-spec approach to Rusty
Spire's existing frontmatter, requirement IDs, offline traceability manifest,
generated schemas, and CI check registry.

## References

- [Rumoca specification guidelines](https://github.com/CogniPilot/rumoca/blob/main/spec/SPEC_0000_SPEC_GUIDELINES.md) (retrieved 2026-08-12)
- [Rumoca agent routing index](https://github.com/CogniPilot/rumoca/blob/main/AGENTS.md) (retrieved 2026-08-12)
- [RFC 2119 requirement keywords](https://www.rfc-editor.org/rfc/rfc2119)
- [RFC 8174 uppercase clarification](https://www.rfc-editor.org/rfc/rfc8174)
