# Rusty Spire specifications

This directory is the normative source for Rusty Spire behavior and
architecture. Accepted requirements bind v0.3; draft requirements describe
non-binding proposals. Explanatory material under `docs/`, the root README,
and `AGENTS.md` may route or explain, but cannot override an accepted spec.

Start with [SPEC-000](000-specification-guidelines.md), then load only the
domain specification needed for a task and its direct dependencies. Every
accepted requirement maps through [traceability.json](traceability.json) to a
command in [checks.json](checks.json).

Card and relic realization/publication status is tracked in the
[content ledgers](content/README.md).

## Accepted specifications

| Spec | Title | Domain | Status | Requirements | Depends | Lines |
|---|---|---|---|---|---|---:|
| [SPEC-000](000-specification-guidelines.md) | Specification Format and Governance | governance | ACCEPTED | GOV-001–GOV-012 | — | 347 |
| [SPEC-001](001-project.md) | Project Scope and Correctness Policy | product | ACCEPTED | PRJ-001–PRJ-004 | SPEC-000 | 162 |
| [SPEC-002](002-architecture.md) | Architecture and Crate Boundaries | architecture | ACCEPTED | ARC-001–ARC-004 | SPEC-001 | 211 |
| [SPEC-003](003-domain.md) | Combat Domain and State Invariants | domain | ACCEPTED | DOM-001–DOM-004 | SPEC-002 | 246 |
| [SPEC-004](004-data.md) | Spire Codex Evidence and Data Packages | data | ACCEPTED | DAT-001–DAT-004 | SPEC-001, SPEC-002 | 242 |
| [SPEC-005](005-combat.md) | Combat Initialization and Transition Semantics | combat | ACCEPTED | CMB-001–CMB-005 | SPEC-003, SPEC-004 | 286 |
| [SPEC-006](006-search.md) | Exact Search and Proof Semantics | search | ACCEPTED | SRCH-001–SRCH-004 | SPEC-003, SPEC-005 | 235 |
| [SPEC-007](007-interfaces.md) | Versioned Application and Wire Interfaces | interfaces | ACCEPTED | API-001–API-005 | SPEC-001, SPEC-002, SPEC-003, SPEC-005, SPEC-006 | 201 |
| [SPEC-008](008-products.md) | CLI Web and Data Tool Responsibilities | applications | ACCEPTED | APP-001–APP-003 | SPEC-001, SPEC-002, SPEC-004, SPEC-007 | 141 |
| [SPEC-010](010-conformance.md) | Conformance and CI Policy | quality | ACCEPTED | CI-001–CI-006 | SPEC-000, SPEC-001, SPEC-002, SPEC-004, SPEC-007, SPEC-008 | 167 |

## Draft specifications

| Spec | Title | Domain | Status | Requirements | Depends | Lines |
|---|---|---|---|---|---|---:|
| [SPEC-009](009-bridge-draft.md) | Archived STS2 Evidence Bridge Reactivation | bridge-security | DRAFT | BRI-001–BRI-004 | SPEC-001, SPEC-003, SPEC-004, SPEC-007 | 127 |

Draft requirements have no traceability mappings and do not authorize
implementation or archive reactivation. Status lifecycle, amendment rules,
metadata, required sections, and size budgets are defined only by SPEC-000.

## Validation

Run the offline governance gates from the repository root:

```bash
python3 tools/specs/check.py
python3 tools/specs/generate_contracts.py --check
python3 tools/specs/generate_content_status.py --check
python3 tools/spire-codex/verify.py
python3 tools/specs/check_architecture.py
```

The specification checker validates metadata, section order, requirement IDs,
links, dependencies, status, active-set budgets, index rows, registered test
symbols, and accepted-requirement traceability.
