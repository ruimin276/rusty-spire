#!/usr/bin/env python3
"""Validate implementation coverage and generate the published content ledger."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
IMPLEMENTED = ROOT / "specs/content/implemented-v1.json"
PUBLISHED = ROOT / "specs/content/published-v1.json"
PACKAGE = ROOT / "packages/spire-codex-stable-v0.107.1.json"
CHECKS = ROOT / "specs/checks.json"
TRACEABILITY = ROOT / "specs/traceability.json"
IMPLEMENTATION_STATUSES = {"implemented", "recognized_inert", "represented_only"}


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path.relative_to(ROOT)} must contain an object")
    return value


def require_string_list(entry: dict[str, Any], field: str, content_id: str) -> list[str]:
    value = entry.get(field)
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        raise ValueError(f"{content_id}.{field} must be a string list")
    return value


def validate_evidence(
    content_id: str,
    entry: dict[str, Any],
    registered_checks: set[str],
    traceability: dict[str, list[str]],
) -> None:
    requirements = require_string_list(entry, "requirements", content_id)
    checks = require_string_list(entry, "checks", content_id)
    if not requirements or not checks:
        raise ValueError(f"{content_id} must name requirements and checks")
    unknown_requirements = set(requirements) - traceability.keys()
    unknown_checks = set(checks) - registered_checks
    if unknown_requirements:
        raise ValueError(f"{content_id} names unknown requirements: {sorted(unknown_requirements)}")
    if unknown_checks:
        raise ValueError(f"{content_id} names unknown checks: {sorted(unknown_checks)}")
    mapped = {check for requirement in requirements for check in traceability[requirement]}
    if not set(checks) <= mapped:
        raise ValueError(f"{content_id} checks are not mapped by its requirements")


def validate_implemented(
    implemented: dict[str, Any],
    package: dict[str, Any],
    checks: dict[str, Any],
    traceability: dict[str, Any],
) -> None:
    if implemented.get("schema_version") != 1 or implemented.get("kind") != "implemented_content":
        raise ValueError("implemented content ledger must use implemented_content schema version 1")
    registered_checks = set(checks.get("checks", {}))
    mappings = traceability.get("requirements", {})
    cards = implemented.get("cards")
    relics = implemented.get("relics")
    if not isinstance(cards, dict) or not set(package["cards"]) <= set(cards):
        raise ValueError("every published card must have an implementation status")
    if not isinstance(relics, dict) or not set(package["items"]["relics"]) <= set(relics):
        raise ValueError("every published relic must have an implementation status")

    for content_id, entry in cards.items():
        published = package["cards"].get(content_id)
        if not isinstance(entry.get("name"), str) or not entry["name"]:
            raise ValueError(f"{content_id} must have a name")
        if published is not None and entry["name"] != published.get("name"):
            raise ValueError(f"{content_id} name disagrees with the package")
        status = entry.get("implementation_status")
        if status not in IMPLEMENTATION_STATUSES:
            raise ValueError(f"{content_id} has invalid implementation_status")
        levels = entry.get("upgrade_levels")
        if levels != [0, 1]:
            raise ValueError(f"{content_id} must explicitly cover upgrade levels 0 and 1")
        declared_effects = require_string_list(entry, "declared_effects", content_id)
        published_effects = (
            [effect["type"] for effect in published.get("effects", [])]
            if published is not None
            else declared_effects
        )
        if declared_effects != published_effects:
            raise ValueError(f"{content_id} declared effect order disagrees with the package")
        mechanics = require_string_list(entry, "realized_mechanics", content_id)
        if status == "implemented" and not mechanics:
            raise ValueError(f"{content_id} is implemented but realizes no mechanics")
        validate_evidence(content_id, entry, registered_checks, mappings)

    for content_id, entry in relics.items():
        published = package["items"]["relics"].get(content_id)
        if not isinstance(entry.get("name"), str) or not entry["name"]:
            raise ValueError(f"{content_id} must have a name")
        if published is not None and entry["name"] != published.get("name"):
            raise ValueError(f"{content_id} name disagrees with the package")
        status = entry.get("implementation_status")
        if status not in IMPLEMENTATION_STATUSES:
            raise ValueError(f"{content_id} has invalid implementation_status")
        if not isinstance(entry.get("declared_effect"), str) or not entry["declared_effect"]:
            raise ValueError(f"{content_id} must have a declared_effect")
        if (
            published is not None
            and entry["declared_effect"] != published["combat_effect"].get("type")
        ):
            raise ValueError(f"{content_id} declared effect disagrees with the package")
        mechanics = require_string_list(entry, "realized_mechanics", content_id)
        limitations = require_string_list(entry, "limitations", content_id)
        if status == "implemented" and not mechanics:
            raise ValueError(f"{content_id} is implemented but realizes no mechanics")
        if status != "implemented" and not limitations:
            raise ValueError(f"{content_id} needs a limitation for status {status}")
        validate_evidence(content_id, entry, registered_checks, mappings)


def published_document() -> dict[str, Any]:
    implemented = load(IMPLEMENTED)
    package_bytes = PACKAGE.read_bytes()
    package = json.loads(package_bytes)
    checks = load(CHECKS)
    traceability = load(TRACEABILITY)
    validate_implemented(implemented, package, checks, traceability)

    cards = {
        content_id: {
            "effects": definition.get("effects", []),
            "implementation_status": implemented["cards"][content_id]["implementation_status"],
            "keywords": definition.get("keywords", []),
            "name": definition["name"],
            "upgrade_levels": implemented["cards"][content_id]["upgrade_levels"],
        }
        for content_id, definition in package["cards"].items()
    }
    relics = {
        content_id: {
            "combat_effect": definition["combat_effect"],
            "implementation_status": implemented["relics"][content_id]["implementation_status"],
            "name": definition["name"],
        }
        for content_id, definition in package["items"]["relics"].items()
    }
    manifest = package["manifest"]
    return {
        "cards": cards,
        "generated_from": [
            str(IMPLEMENTED.relative_to(ROOT)),
            str(PACKAGE.relative_to(ROOT)),
        ],
        "implementation_contract": str(IMPLEMENTED.relative_to(ROOT)),
        "kind": "published_content",
        "package": {
            "game_version": manifest["source"]["game_version"],
            "package_id": manifest["package_id"],
            "path": str(PACKAGE.relative_to(ROOT)),
            "sha256": hashlib.sha256(package_bytes).hexdigest(),
            "source_channel": manifest["source"]["channel"],
        },
        "relics": relics,
        "schema_version": 1,
    }


def encoded() -> str:
    return json.dumps(published_document(), indent=2, sort_keys=True) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    expected = encoded()
    if args.check:
        if not PUBLISHED.exists() or PUBLISHED.read_text(encoding="utf-8") != expected:
            raise SystemExit(f"published content ledger is stale: {PUBLISHED.relative_to(ROOT)}")
        action = "verified"
    else:
        PUBLISHED.parent.mkdir(parents=True, exist_ok=True)
        PUBLISHED.write_text(expected, encoding="utf-8")
        action = "generated"
    document = json.loads(expected)
    print(f"{action} {len(document['cards'])} cards and {len(document['relics'])} relics")


if __name__ == "__main__":
    main()
