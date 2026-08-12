# Simulator guide

The normative combat, search, and interface contracts are
[`SPEC-005`](../specs/005-combat.md), [`SPEC-006`](../specs/006-search.md), and
[`SPEC-007`](../specs/007-interfaces.md). This page is an implementation guide.

`rusty-spire-core` represents branchable combat state and named RNG streams.
`rusty-spire-data` validates the selected content package.
`rusty-spire-combat::CombatEngine` initializes combat, enumerates legal actions,
applies immutable transitions, and calculates canonical state IDs.
`rusty-spire-simulator` performs graph search over that interface.

The default exact objective minimizes combat-start HP minus current/final HP.
Limits produce incomplete results and never prove optimality. Heuristics are
ordering inputs; approximate mode is explicit and always clears
`optimality_proven`.

The v0.3 API separates combat input (`CombatSetupV2`) from search configuration
(`SolveRequestV1`). A setup identifies exact package bytes by package ID and
SHA-256. Browser and native callers receive the same structured error codes.

The Web application uses the WASM `sls2_call_v1` dispatcher. The legacy
`CombatSetupV1`, CLI flags, response shape, and `sls2_solve_json` remain adapters
for v0.3 only.

Executable card behavior uses an ordered, closed effect vocabulary in the data
package. Static values originate in committed Spire Codex evidence; effect
semantics and ordering are reviewed separately. The four v0.3 proof cards cover
block-then-damage, block-then-draw, damage-then-draw, and
energy-then-draw-then-exhaust composition.
