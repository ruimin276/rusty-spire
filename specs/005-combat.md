---
id: SPEC-005
title: Combat initialization and transitions
status: accepted
depends: [SPEC-003, SPEC-004]
---

# Combat initialization and transitions

### CMB-001 — Initialization

Initialization validates package identity and setup capability before creating
stable card instances, RNG streams, enemies, relic state, and the opening hand.

### CMB-002 — Effect vocabulary

Promoted cards use a closed reviewed effect vocabulary. Effects execute in
declared order; unsupported effect declarations fail package validation.

### CMB-003 — Turn lifecycle

Card payment, resolution, pile destination, end-turn cleanup, enemy actions,
power ticking, energy reset, and drawing occur in the documented deterministic
order and stop immediately at a terminal state.

### CMB-004 — Proof-slice cards

Iron Wave executes block 5/7 then damage 5/7. Backflip executes block 5/8 then
draw 2. Pommel Strike executes damage 9/10 then draw 1/2. Adrenaline executes
energy 1/2 then draw 2 and exhausts after resolution.
