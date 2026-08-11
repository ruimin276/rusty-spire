from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


class CatalogBuildError(RuntimeError):
    pass


COLLECTION_PREFIXES = {
    "cards": "CARD.",
    "characters": "CHARACTER.",
    "relics": "RELIC.",
    "monsters": "MONSTER.",
    "encounters": "ENCOUNTER.",
    "powers": "POWER.",
    "ascensions": "ASCENSION.",
}


def _load_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise CatalogBuildError(f"cannot read {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise CatalogBuildError(f"{path} must contain a JSON object")
    return value


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _source_records(snapshot: Path, collection: str) -> dict[str, dict[str, Any]]:
    path = snapshot / "raw" / f"{collection}.json"
    try:
        values = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise CatalogBuildError(f"cannot read raw {collection}: {exc}") from exc
    if not isinstance(values, list):
        raise CatalogBuildError(f"raw {collection} must be an array")
    result = {}
    for value in values:
        if not isinstance(value, dict) or not isinstance(value.get("id"), str):
            raise CatalogBuildError(f"raw {collection} has an invalid id")
        model_id = f"{COLLECTION_PREFIXES[collection]}{value['id']}"
        if model_id in result:
            raise CatalogBuildError(
                f"raw {collection} contains duplicate {model_id}"
            )
        result[model_id] = value
    return result


def _require_equal(entity: str, field: str, reviewed: Any, source: Any) -> None:
    if reviewed != source:
        raise CatalogBuildError(
            f"reviewed {entity} {field}={reviewed!r} does not match snapshot {source!r}"
        )


def _upgrade_delta(record: dict[str, Any], field: str, entity: str) -> int:
    upgrade = record.get("upgrade")
    value = upgrade.get(field) if isinstance(upgrade, dict) else None
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        try:
            return int(value)
        except ValueError:
            pass
    raise CatalogBuildError(f"snapshot {entity} has no numeric {field} upgrade")


def _validate_card_values(
    catalog: dict[str, Any], records: dict[str, dict[str, dict[str, Any]]]
) -> None:
    powers = catalog["powers"]
    for model_id, definition in catalog["cards"].items():
        source = records["cards"][model_id]
        _require_equal(model_id, "cost", definition.get("cost"), source.get("cost"))
        for field in ("damage", "block"):
            reviewed = definition.get(field)
            source_base = source.get(field)
            if reviewed is None:
                if source_base is not None:
                    raise CatalogBuildError(
                        f"reviewed {model_id} omits snapshot {field}={source_base!r}"
                    )
                continue
            if not isinstance(reviewed, dict) or not isinstance(source_base, int):
                raise CatalogBuildError(f"reviewed {model_id} has invalid {field}")
            _require_equal(model_id, f"{field}.base", reviewed.get("base"), source_base)
            expected_upgrade = source_base + _upgrade_delta(source, field, model_id)
            _require_equal(
                model_id,
                f"{field}.upgraded",
                reviewed.get("upgraded"),
                expected_upgrade,
            )

        reviewed_keywords = sorted(definition.get("keywords", []))
        source_keywords = sorted(source.get("keywords") or [])
        _require_equal(model_id, "keywords", reviewed_keywords, source_keywords)

        reviewed_power = definition.get("power")
        source_powers = source.get("powers_applied") or []
        if reviewed_power is None:
            if source_powers:
                raise CatalogBuildError(
                    f"reviewed {model_id} omits snapshot powers_applied"
                )
            continue
        if not isinstance(reviewed_power, dict):
            raise CatalogBuildError(f"reviewed {model_id} has invalid power")
        power_definition = powers.get(reviewed_power.get("id"))
        if not isinstance(power_definition, dict):
            raise CatalogBuildError(f"reviewed {model_id} references an unknown power")
        source_power_id = power_definition.get("source_id")
        source_power = next(
            (
                power
                for power in source_powers
                if isinstance(power, dict)
                and str(power.get("power_key", power.get("power_id", ""))).upper()
                == source_power_id
            ),
            None,
        )
        if source_power is None:
            raise CatalogBuildError(
                f"snapshot {model_id} has no power {source_power_id}"
            )
        amount = reviewed_power.get("amount")
        source_amount = source_power.get("amount")
        if not isinstance(amount, dict) or not isinstance(source_amount, int):
            raise CatalogBuildError(f"reviewed {model_id} has invalid power amount")
        _require_equal(model_id, "power.amount.base", amount.get("base"), source_amount)
        upgrade_field = str(source_power_id).lower()
        expected_upgrade = source_amount + _upgrade_delta(
            source, upgrade_field, model_id
        )
        _require_equal(
            model_id,
            "power.amount.upgraded",
            amount.get("upgraded"),
            expected_upgrade,
        )


def _validate_character_values(
    catalog: dict[str, Any], records: dict[str, dict[str, dict[str, Any]]]
) -> None:
    for model_id, definition in catalog["characters"].items():
        _require_equal(
            model_id,
            "max_energy",
            definition.get("max_energy"),
            records["characters"][model_id].get("max_energy"),
        )


def _validate_monster_values(
    catalog: dict[str, Any], records: dict[str, dict[str, dict[str, Any]]]
) -> None:
    for model_id, definition in catalog["monsters"].items():
        source = records["monsters"][model_id]
        for reviewed_field, source_min, source_max in (
            ("hp", "min_hp", "max_hp"),
            ("ascension_hp", "min_hp_ascension", "max_hp_ascension"),
        ):
            reviewed_range = definition.get(reviewed_field)
            if not isinstance(reviewed_range, dict):
                raise CatalogBuildError(
                    f"reviewed {model_id} has invalid {reviewed_field}"
                )
            _require_equal(
                model_id, f"{reviewed_field}.min", reviewed_range.get("min"), source.get(source_min)
            )
            _require_equal(
                model_id, f"{reviewed_field}.max", reviewed_range.get("max"), source.get(source_max)
            )

        source_moves = {
            f"{movement['id']}_MOVE": movement
            for movement in source.get("moves", [])
            if isinstance(movement, dict) and isinstance(movement.get("id"), str)
        }
        for move_id, movement in definition.get("moves", {}).items():
            source_move = source_moves.get(move_id)
            if source_move is None:
                raise CatalogBuildError(f"snapshot {model_id} has no move {move_id}")
            source_damage = source_move.get("damage")
            reviewed_damage = movement.get("damage")
            if source_damage is not None:
                if not isinstance(reviewed_damage, dict) or not isinstance(source_damage, dict):
                    raise CatalogBuildError(f"reviewed {model_id} {move_id} omits damage")
                _require_equal(
                    f"{model_id} {move_id}",
                    "damage.base",
                    reviewed_damage.get("base"),
                    source_damage.get("normal"),
                )
                _require_equal(
                    f"{model_id} {move_id}",
                    "damage.ascension",
                    reviewed_damage.get("ascension"),
                    source_damage.get("ascension"),
                )
            elif reviewed_damage is not None:
                raise CatalogBuildError(
                    f"reviewed {model_id} {move_id} adds damage absent from snapshot"
                )

            source_block = source_move.get("block")
            if source_block is not None:
                reviewed_block = movement.get("block")
                if not isinstance(reviewed_block, dict):
                    raise CatalogBuildError(f"reviewed {model_id} {move_id} omits block")
                _require_equal(
                    f"{model_id} {move_id}",
                    "block.base",
                    reviewed_block.get("base"),
                    source_block,
                )

            source_powers = source_move.get("powers") or []
            if source_powers:
                reviewed_power = movement.get("power")
                if not isinstance(reviewed_power, dict) or len(source_powers) != 1:
                    raise CatalogBuildError(f"reviewed {model_id} {move_id} omits power")
                source_power = source_powers[0]
                power_definition = catalog["powers"].get(reviewed_power.get("id"))
                if not isinstance(power_definition, dict):
                    raise CatalogBuildError(
                        f"reviewed {model_id} {move_id} references an unknown power"
                    )
                _require_equal(
                    f"{model_id} {move_id}",
                    "power.id",
                    power_definition.get("source_id"),
                    source_power.get("power_id"),
                )
                _require_equal(
                    f"{model_id} {move_id}",
                    "power.amount.base",
                    reviewed_power.get("amount", {}).get("base"),
                    source_power.get("amount"),
                )
                _require_equal(
                    f"{model_id} {move_id}",
                    "power.target",
                    reviewed_power.get("target"),
                    source_power.get("target"),
                )


def _validate_encounter_values(
    catalog: dict[str, Any], records: dict[str, dict[str, dict[str, Any]]]
) -> None:
    for model_id, definition in catalog["encounters"].items():
        source_values = records["encounters"][model_id].get("monsters", [])
        source_enemies = [
            f"MONSTER.{value['id'] if isinstance(value, dict) else value}"
            for value in source_values
        ]
        reviewed_enemies = definition.get("enemies", [])
        collapsed_reviewed = [
            value
            for index, value in enumerate(reviewed_enemies)
            if index == 0 or value != reviewed_enemies[index - 1]
        ]
        _require_equal(model_id, "enemies", collapsed_reviewed, source_enemies)


def _validate_ascension_values(
    catalog: dict[str, Any], records: dict[str, dict[str, dict[str, Any]]]
) -> None:
    rules = catalog.get("ascensions")
    if not isinstance(rules, dict):
        raise CatalogBuildError("reviewed catalog ascensions must be an object")
    ascensions = list(records["ascensions"].values())
    max_level = max((record.get("level", -1) for record in ascensions), default=-1)
    _require_equal(
        "ascensions", "max_supported_level", rules.get("max_supported_level"), max_level
    )
    tough_level = rules.get("tough_enemies_level")
    deadly_level = rules.get("deadly_enemies_level")
    monster_hp_level = rules.get("monster_hp_level")
    _require_equal("ascensions", "monster_hp_level", monster_hp_level, tough_level)
    by_level = {record.get("level"): record for record in ascensions}
    if "tough" not in str(by_level.get(tough_level, {}).get("name", "")).lower():
        raise CatalogBuildError("snapshot has no Tough Enemies ascension at the reviewed level")
    if "deadly" not in str(by_level.get(deadly_level, {}).get("name", "")).lower():
        raise CatalogBuildError("snapshot has no Deadly Enemies ascension at the reviewed level")


def _validate_static_values(
    catalog: dict[str, Any], records: dict[str, dict[str, dict[str, Any]]]
) -> None:
    _validate_card_values(catalog, records)
    _validate_character_values(catalog, records)
    _validate_monster_values(catalog, records)
    _validate_encounter_values(catalog, records)
    _validate_ascension_values(catalog, records)


def build_catalog(snapshot: Path, reviewed: Path, output: Path) -> dict[str, Any]:
    """Promote a reviewed behavior slice from exactly one immutable snapshot."""
    manifest = _load_object(snapshot / "manifest.json")
    catalog = _load_object(reviewed)
    source = manifest.get("source")
    if not isinstance(source, dict) or source.get("channel") not in {"stable", "beta"}:
        raise CatalogBuildError("snapshot manifest has no valid source channel")
    reviewed_source = catalog.get("source")
    if not isinstance(reviewed_source, dict):
        raise CatalogBuildError("reviewed catalog has no source object")
    if reviewed_source.get("channel") != source["channel"]:
        raise CatalogBuildError("reviewed catalog and snapshot channels differ")

    endpoints = manifest.get("endpoints")
    if not isinstance(endpoints, dict):
        raise CatalogBuildError("snapshot manifest has no endpoints object")
    for endpoint, metadata in endpoints.items():
        if not isinstance(metadata, dict) or not isinstance(metadata.get("sha256"), str):
            raise CatalogBuildError(f"manifest metadata for {endpoint} is invalid")
        raw = snapshot / "raw" / f"{endpoint}.json"
        try:
            actual = _sha256(raw.read_bytes())
        except OSError as exc:
            raise CatalogBuildError(f"cannot read raw endpoint {endpoint}: {exc}") from exc
        if actual != metadata["sha256"]:
            raise CatalogBuildError(f"raw endpoint hash mismatch for {endpoint}")
    endpoint_hashes = {
        endpoint: metadata["sha256"] for endpoint, metadata in endpoints.items()
    }
    aggregate = _sha256(
        json.dumps(endpoint_hashes, sort_keys=True, separators=(",", ":")).encode(
            "utf-8"
        )
    )
    if aggregate != manifest.get("content_sha256"):
        raise CatalogBuildError("snapshot aggregate content hash mismatch")

    records = {
        collection: _source_records(snapshot, collection)
        for collection in COLLECTION_PREFIXES
    }
    for collection in ("cards", "characters", "relics", "monsters", "encounters"):
        definitions = catalog.get(collection)
        if not isinstance(definitions, dict):
            raise CatalogBuildError(f"reviewed catalog {collection} must be an object")
        missing = sorted(set(definitions) - records[collection].keys())
        if missing:
            raise CatalogBuildError(
                f"reviewed {collection} IDs absent from snapshot: {', '.join(missing)}"
            )

    referenced_powers = {
        card["power"]["id"]
        for card in catalog["cards"].values()
        if isinstance(card, dict) and isinstance(card.get("power"), dict)
    }
    for monster in catalog["monsters"].values():
        for movement in monster.get("moves", {}).values():
            if isinstance(movement, dict) and isinstance(movement.get("power"), dict):
                referenced_powers.add(movement["power"]["id"])
    reviewed_powers = catalog.get("powers")
    if not isinstance(reviewed_powers, dict):
        raise CatalogBuildError("reviewed catalog powers must be an object")
    referenced_source_powers = set()
    for power_id in referenced_powers:
        definition = reviewed_powers.get(power_id)
        if not isinstance(definition, dict) or not isinstance(
            definition.get("source_id"), str
        ):
            raise CatalogBuildError(f"reviewed power {power_id} has no source_id")
        referenced_source_powers.add(f"POWER.{definition['source_id']}")
    missing_powers = sorted(referenced_source_powers - records["powers"].keys())
    if missing_powers:
        raise CatalogBuildError(
            f"reviewed power IDs absent from snapshot: {', '.join(missing_powers)}"
        )

    _validate_static_values(catalog, records)

    version = manifest.get("version_evidence")
    game_version = version.get("game_version") if isinstance(version, dict) else None
    if not isinstance(game_version, str) or not game_version:
        raise CatalogBuildError("snapshot does not report a game version")
    content_sha256 = manifest.get("content_sha256")
    retrieved_at = manifest.get("retrieved_at")
    if not isinstance(content_sha256, str) or not isinstance(retrieved_at, str):
        raise CatalogBuildError("snapshot provenance is incomplete")

    catalog["source"] = {
        "name": "spire_codex",
        "channel": source["channel"],
        "game_version": game_version,
        "content_sha256": content_sha256,
        "retrieved_at": retrieved_at,
    }
    encoded = (
        json.dumps(catalog, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded)
    return {"output": str(output), "sha256": _sha256(encoded), "source": catalog["source"]}


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Build CombatCatalogV1 from one immutable snapshot and a reviewed slice."
    )
    parser.add_argument("--snapshot", type=Path, required=True)
    parser.add_argument("--reviewed", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        result = build_catalog(args.snapshot, args.reviewed, args.output)
    except CatalogBuildError as exc:
        parser.exit(1, f"catalog build failed: {exc}\n")
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
