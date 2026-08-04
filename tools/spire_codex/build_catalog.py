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


def _source_ids(snapshot: Path, collection: str) -> set[str]:
    path = snapshot / "normalized" / f"{collection}.json"
    try:
        values = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise CatalogBuildError(f"cannot read normalized {collection}: {exc}") from exc
    if not isinstance(values, list):
        raise CatalogBuildError(f"normalized {collection} must be an array")
    result = set()
    for value in values:
        if not isinstance(value, dict) or not isinstance(value.get("model_id"), str):
            raise CatalogBuildError(f"normalized {collection} has an invalid model_id")
        if value["model_id"] in result:
            raise CatalogBuildError(
                f"normalized {collection} contains duplicate {value['model_id']}"
            )
        result.add(value["model_id"])
    return result


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

    available = {
        collection: _source_ids(snapshot, collection)
        for collection in COLLECTION_PREFIXES
    }
    for collection in ("cards", "characters", "relics", "monsters", "encounters"):
        definitions = catalog.get(collection)
        if not isinstance(definitions, dict):
            raise CatalogBuildError(f"reviewed catalog {collection} must be an object")
        missing = sorted(set(definitions) - available[collection])
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
    missing_powers = sorted(referenced_source_powers - available["powers"])
    if missing_powers:
        raise CatalogBuildError(
            f"reviewed power IDs absent from snapshot: {', '.join(missing_powers)}"
        )

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
