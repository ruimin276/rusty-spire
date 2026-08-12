#!/usr/bin/env python3
"""Promote compact Spire Codex evidence into the canonical v0.3 data package."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_EVIDENCE = ROOT / "evidence/spire-codex-stable-v0.107.1-selected.json"
DEFAULT_REVIEW = ROOT / "packages/reviewed-effects-v1.json"
DEFAULT_BASE = ROOT / "catalogs/combat_v0.107.1.json"
DEFAULT_OUTPUT = ROOT / "packages/spire-codex-stable-v0.107.1.json"


class PromotionError(RuntimeError):
    pass


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise PromotionError(f"{path} must contain an object")
    return value


def delta(record: dict[str, Any], key: str) -> int:
    raw = (record.get("upgrade") or {}).get(key, 0)
    if isinstance(raw, int):
        return raw
    try:
        return int(str(raw))
    except ValueError as error:
        raise PromotionError(f"{record['id']} has non-numeric {key} upgrade") from error


def amount(record: dict[str, Any], field: str, upgrade_key: str | None = None) -> dict[str, int]:
    base = record.get(field)
    if not isinstance(base, int):
        raise PromotionError(f"{record['id']} has no numeric {field}")
    return {"base": base, "upgraded": base + delta(record, upgrade_key or field)}


def card_definition(
    model_id: str,
    record: dict[str, Any],
    selection: dict[str, Any],
    base: dict[str, Any] | None,
    asset: str | None,
) -> dict[str, Any]:
    definition: dict[str, Any] = {
        "cost": record["cost"],
        "name": record["name"],
        "character": selection.get("character"),
        "card_type": selection.get("type"),
        "keywords": record.get("keywords_key") or record.get("keywords") or [],
        "effects": [],
    }
    if asset:
        definition["asset"] = asset
    if isinstance(record.get("damage"), int):
        definition["damage"] = amount(record, "damage")
    if isinstance(record.get("block"), int):
        definition["block"] = amount(record, "block")
    if base and "power" in base:
        definition["power"] = base["power"]

    for effect in selection["effects"]:
        if effect == "damage":
            definition["effects"].append({"type": "damage", "amount": definition["damage"]})
        elif effect == "block":
            definition["effects"].append({"type": "block", "amount": definition["block"]})
        elif effect == "draw":
            definition["effects"].append(
                {"type": "draw", "amount": amount(record, "cards_draw", "cards")}
            )
        elif effect == "energy":
            definition["effects"].append(
                {"type": "energy", "amount": amount(record, "energy_gain", "energy")}
            )
        elif effect == "discard":
            definition["effects"].append({"type": "discard", "amount": 1})
        elif effect == "power":
            if not base or not isinstance(base.get("power"), dict):
                raise PromotionError(f"{model_id} has no reviewed power mapping")
            definition["effects"].append(
                {
                    "type": "apply_power",
                    "id": base["power"]["id"],
                    "amount": base["power"]["amount"],
                }
            )
        else:
            raise PromotionError(f"{model_id} has unsupported reviewed effect {effect}")
    return definition


def promote(evidence_path: Path, review_path: Path, base_path: Path) -> dict[str, Any]:
    evidence = load(evidence_path)
    review = load(review_path)
    base = load(base_path)
    if evidence.get("schema_version") != 1 or review.get("schema_version") != 1:
        raise PromotionError("evidence and review schema_version must be 1")

    card_records = {f"CARD.{record['id']}": record for record in evidence["cards"]}
    reviewed_cards = review["cards"]
    if card_records.keys() != reviewed_cards.keys():
        missing = sorted(reviewed_cards.keys() - card_records.keys())
        extra = sorted(card_records.keys() - reviewed_cards.keys())
        raise PromotionError(f"review/evidence card mismatch; missing={missing}, extra={extra}")
    cards = {
        model_id: card_definition(
            model_id,
            card_records[model_id],
            selection,
            base["cards"].get(model_id),
            review.get("assets", {}).get(model_id),
        )
        for model_id, selection in sorted(reviewed_cards.items())
    }

    characters = json.loads(json.dumps(base["characters"]))
    for model_id, reviewed in review["characters"].items():
        if model_id not in characters:
            raise PromotionError(f"unknown reviewed character {model_id}")
        characters[model_id].update(reviewed)

    monster_records = {f"MONSTER.{record['id']}": record for record in evidence["monsters"]}
    monsters = json.loads(json.dumps(base["monsters"]))
    for model_id, definition in monsters.items():
        record = monster_records.get(model_id)
        if not record:
            raise PromotionError(f"no evidence for {model_id}")
        definition["name"] = record["name"]
        asset = review.get("assets", {}).get(model_id)
        if asset:
            definition["asset"] = asset

    relic_records = {f"RELIC.{record['id']}": record for record in evidence["relics"]}
    relics = json.loads(json.dumps(base["relics"]))
    for model_id, definition in relics.items():
        record = relic_records.get(model_id)
        if not record:
            raise PromotionError(f"no evidence for {model_id}")
        definition["name"] = record["name"]
        asset = review.get("assets", {}).get(model_id)
        if asset:
            definition["asset"] = asset

    return {
        "manifest": {
            "schema_version": 1,
            "package_id": "spire-codex-stable-v0.107.1",
            "source": base["source"],
        },
        "cards": cards,
        "actors": {"characters": characters, "monsters": monsters},
        "items": {"relics": relics, "powers": base["powers"]},
        "encounters": base["encounters"],
        "rules": {
            "rng_profiles": base["rng_profiles"],
            "ascensions": base["ascensions"],
            "combat_modifiers": base["combat_modifiers"],
        },
    }


def encoded(package: dict[str, Any]) -> bytes:
    return (json.dumps(package, indent=2, sort_keys=True) + "\n").encode()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", type=Path, default=DEFAULT_EVIDENCE)
    parser.add_argument("--reviewed", type=Path, default=DEFAULT_REVIEW)
    parser.add_argument("--base", type=Path, default=DEFAULT_BASE)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    package = promote(args.evidence, args.reviewed, args.base)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(encoded(package))
    print(args.output)


if __name__ == "__main__":
    main()
