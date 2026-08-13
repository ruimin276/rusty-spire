#!/usr/bin/env python3
"""Generate JSON Schema and browser TypeScript contract declarations."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCHEMAS = ROOT / "specs/schemas"
TYPES = ROOT / "apps/web/src/contracts.generated.ts"

OBJECT = {"type": "object", "additionalProperties": False}
PACKAGE = {
    **OBJECT,
    "required": ["package_id", "sha256"],
    "properties": {"package_id": {"type": "string", "minLength": 1}, "sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"}},
}

SCHEMA_DOCUMENTS = {
    "combat-setup-v2.schema.json": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://rusty-spire.dev/schemas/combat-setup-v2.json",
        **OBJECT,
        "required": ["schema_version", "package", "ascension_level", "rng", "character", "deck", "relics", "potions", "encounter"],
        "properties": {
            "schema_version": {"const": 2}, "package": PACKAGE,
            "ascension_level": {"type": "integer", "minimum": 0},
            "rng": {"type": "object"}, "character": {"type": "object"},
            "deck": {"type": "array", "minItems": 1}, "relics": {"type": "array"},
            "potions": {"type": "array", "maxItems": 0}, "encounter": {"type": "object"},
        },
    },
    "solve-request-v1.schema.json": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://rusty-spire.dev/schemas/solve-request-v1.json",
        **OBJECT,
        "required": ["schema_version", "setup"],
        "properties": {
            "schema_version": {"const": 1},
            "setup": {"$ref": "combat-setup-v2.schema.json"},
            "policy": {"const": "minimize_hp_loss"},
            "mode": {"enum": ["exact", "approximate"]},
            "heuristic": {"enum": ["zero", "remaining_enemy_hp"]},
            "limits": {"type": "object"},
            "include_replay": {"type": "boolean"},
        },
    },
    "solve-response-v1.schema.json": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://rusty-spire.dev/schemas/solve-response-v1.json",
        **OBJECT,
        "required": ["schema_version", "result", "opening_hand", "actions"],
        "properties": {
            "schema_version": {"const": 1},
            "result": {"type": "object"},
            "opening_hand": {"type": "array", "items": {"type": "string"}},
            "actions": {"type": "array", "items": {"type": "object"}},
            "replay": {
                **OBJECT,
                "required": ["frames"],
                "properties": {
                    "frames": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            **OBJECT,
                            "required": ["index", "turn", "action", "state", "resolved_enemy_intents"],
                            "properties": {
                                "index": {"type": "integer", "minimum": 0},
                                "turn": {"type": "integer", "minimum": 1},
                                "action": {"type": ["object", "null"]},
                                "state": {"type": "object"},
                                "resolved_enemy_intents": {"type": "array", "items": {"type": "object"}},
                            },
                        },
                    }
                },
            },
        },
    },
    "api-error-v1.schema.json": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://rusty-spire.dev/schemas/api-error-v1.json",
        **OBJECT,
        "required": ["schema_version", "code", "message"],
        "properties": {
            "schema_version": {"const": 1},
            "code": {"enum": ["invalid_json", "invalid_request", "package_mismatch", "unknown_id", "unsupported", "invalid_action", "internal"]},
            "message": {"type": "string"},
        },
    },
    "content-manifest-v1.schema.json": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://rusty-spire.dev/schemas/content-manifest-v1.json",
        **OBJECT,
        "required": ["schema_version", "package", "game_version", "characters", "cards", "enemies", "relics"],
        "properties": {
            "schema_version": {"const": 1}, "package": PACKAGE,
            "game_version": {"type": "string"}, "characters": {"type": "array"},
            "cards": {"type": "array"}, "enemies": {"type": "array"}, "relics": {"type": "array"},
        },
    },
    "combat-snapshot-v3.schema.json": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://rusty-spire.dev/schemas/combat-snapshot-v3.json",
        **OBJECT,
        "required": ["schema_version", "state"],
        "properties": {"schema_version": {"const": 3}, "state": {"type": "object"}},
    },
}

TYPESCRIPT = """// Generated by tools/specs/generate_contracts.py. Do not edit.
export type CombatAction = { id: string; type: string; card_id: string | null; combat_card_index: string | null; target_combat_id: string | null; cost: number | null; choice_id: string | null; selection: string[] };
export type TraceStep = { action: CombatAction; state_hash: string; hp_loss: number };
export type SolveResult = { catalog_sha256: string; catalog_game_version: string; setup_hash: string; policy: \"minimize_hp_loss\"; won: boolean; complete: boolean; optimality_proven: boolean; hp_loss: number | null; final_hp: number | null; actions: TraceStep[]; action_ids: string[]; explored_states: number; cache_hits: number; runtime_seconds: number; termination_reason: string };
export type PackageIdentity = { package_id: string; sha256: string };
export type ContentCard = { id: string; name: string; character: string | null; card_type: string | null; cost: number; asset: string | null };
export type ContentCharacter = { id: string; name: string; max_hp: number; max_energy: number; starter_deck: Array<{ id: string; quantity: number; upgrade_level: number }>; starter_relics: string[]; asset: string | null };
export type ContentEnemy = { id: string; name: string; hp: [number, number]; ascension_hp: [number, number]; asset: string | null };
export type ContentManifest = { schema_version: 1; package: PackageIdentity; game_version: string; characters: ContentCharacter[]; cards: ContentCard[]; enemies: ContentEnemy[]; relics: Array<{ id: string; name: string; asset: string | null }> };
export type CombatSetupV2 = { schema_version: 2; package: PackageIdentity; ascension_level: number; rng: { run_seed: string; profile: string }; character: { id: string; current_hp: number; max_hp: number }; deck: Array<{ id: string; quantity: number; upgrade_level: number }>; relics: Array<{ id: string }>; potions: never[]; encounter: { type: \"custom\"; enemies: Array<{ id: string; current_hp: number; max_hp: number }> } };
export type ReplayAction = { type: \"card\"; id: string; card_id: string; instance_id: string; target_id: string | null; cost: number } | { type: \"end_turn\"; id: string } | { type: \"choose\"; id: string; choice_id: string; selection: string[] };
export type ReplayPower = { id: string; name: string; amount: number };
export type ReplayCard = { instance_id: string; card_id: string; upgrade_level: number; effective_cost: number };
export type EnemyPowerIntent = { id: string; name: string; amount: number; target: string };
export type EnemyIntent = { enemy_id: string; move_id: string; name: string; damage: number | null; block: number | null; power: EnemyPowerIntent | null };
export type ReplayPlayer = { id: string; model_id: string; hp: number; max_hp: number; block: number; energy: number; max_energy: number; powers: ReplayPower[] };
export type ReplayEnemy = { id: string; model_id: string; hp: number; max_hp: number; block: number; powers: ReplayPower[]; current_intent: EnemyIntent | null };
export type ReplayState = { state_id: string; turn: number; status: \"active\" | \"won\" | \"lost\"; decision: \"player_action\" | \"card_selection\" | \"terminal\"; player: ReplayPlayer; enemies: ReplayEnemy[]; hand: ReplayCard[]; draw_pile: ReplayCard[]; discard_pile: ReplayCard[]; exhaust_pile: ReplayCard[]; play_pile: ReplayCard[] };
export type ReplayFrame = { index: number; turn: number; action: ReplayAction | null; state: ReplayState; resolved_enemy_intents: EnemyIntent[] };
export type CombatReplay = { frames: ReplayFrame[] };
export type BrowserSolveResult = { schema_version: 1; result: SolveResult; opening_hand: string[]; actions: ReplayAction[]; replay?: CombatReplay };
"""


def outputs() -> dict[Path, str]:
    values = {SCHEMAS / name: json.dumps(schema, indent=2, sort_keys=True) + "\n" for name, schema in SCHEMA_DOCUMENTS.items()}
    values[TYPES] = TYPESCRIPT
    return values


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    for path, expected in outputs().items():
        if args.check:
            if not path.exists() or path.read_text(encoding="utf-8") != expected:
                raise SystemExit(f"generated contract is stale: {path.relative_to(ROOT)}")
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(expected, encoding="utf-8")
    print(f"{'verified' if args.check else 'generated'} {len(outputs())} contract artifacts")


if __name__ == "__main__":
    main()
