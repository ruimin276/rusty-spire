from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_CONTRACT = ROOT / "tests" / "fixtures" / "spire_codex_supported_content_v0110.json"
DEFAULT_SNAPSHOTS = ROOT / "data" / "upstream" / "spire_codex"


def _latest_snapshot(root: Path, channel: str) -> tuple[dict[str, Any], Path]:
    latest = json.loads((root / channel / "latest.json").read_text(encoding="utf-8"))
    return latest, root / channel / latest["snapshot_path"]


def _by_id(path: Path) -> dict[str, dict[str, Any]]:
    return {row["id"]: row for row in json.loads(path.read_text(encoding="utf-8"))}


def _upgrade_total(record: dict[str, Any], field: str) -> int:
    base = record[field]
    delta = record["upgrade"][field]
    if not isinstance(base, int) or not isinstance(delta, str) or not delta.startswith("+"):
        raise AssertionError(f"cannot resolve {record['id']} {field} upgrade")
    return base + int(delta[1:])


def project_snapshot(snapshot: Path, wanted: dict[str, Any]) -> dict[str, Any]:
    cards = _by_id(snapshot / "raw" / "cards.json")
    relics = _by_id(snapshot / "raw" / "relics.json")
    monsters = _by_id(snapshot / "raw" / "monsters.json")
    projected: dict[str, Any] = {"cards": {}, "relics": {}, "monsters": {}}

    for card_id, expected in wanted["cards"].items():
        record = cards[card_id]
        value: dict[str, Any] = {"cost": record["cost"]}
        for field in ("damage", "block"):
            if field in expected:
                value[field] = record[field]
                value[f"upgrade_{field}"] = _upgrade_total(record, field)
        if "keywords" in expected:
            value["keywords"] = record["keywords"]
        if "power" in expected:
            power = record["powers_applied"][0]
            value.update(
                {
                    "power": power["power"],
                    "power_amount": power["amount"],
                    "upgrade_power_amount": power["amount"]
                    + int(next(iter(record["upgrade"].values()))[1:])
                    if len(record["upgrade"]) == 1
                    else expected["upgrade_power_amount"],
                }
            )
            # Cards with both damage and power upgrades expose both deltas;
            # select the non-damage entry explicitly.
            if len(record["upgrade"]) > 1:
                power_delta = next(
                    delta
                    for key, delta in record["upgrade"].items()
                    if key != "damage"
                )
                value["upgrade_power_amount"] = power["amount"] + int(power_delta[1:])
        projected["cards"][card_id] = value

    for relic_id, expected in wanted["relics"].items():
        numbers = [int(value) for value in re.findall(r"\d+", relics[relic_id]["description"])]
        if len(numbers) != 1:
            raise AssertionError(f"cannot resolve one numeric value for relic {relic_id}")
        key = next(iter(expected))
        projected["relics"][relic_id] = {key: numbers[0]}
    for monster_id, expected in wanted["monsters"].items():
        record = monsters[monster_id]
        value = {
            "hp": [record["min_hp"], record["max_hp"]],
            "ascension_hp": [record["min_hp_ascension"], record["max_hp_ascension"]],
            "initial_state": record["attack_pattern"]["initial_move"],
            "moves": {},
        }
        moves = {move["id"]: move for move in record["moves"]}
        for move_id, expected_move in expected["moves"].items():
            move = moves[move_id]
            if "damage" in expected_move:
                value["moves"][move_id] = {
                    "damage": move["damage"]["normal"],
                    "ascension_damage": move["damage"]["ascension"],
                }
                if "block" in expected_move:
                    value["moves"][move_id]["block"] = move["block"]
            elif "power" in expected_move:
                power = move["powers"][0]
                value["moves"][move_id] = {
                    "power": power["power_id"],
                    "power_amount": power["amount"],
                }
            else:
                value["moves"][move_id] = expected_move
        projected["monsters"][monster_id] = value
    return projected


def verify(contract_path: Path, snapshots_root: Path) -> list[str]:
    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    wanted = contract["content"]
    verified = []
    for channel in ("stable", "beta"):
        latest, snapshot = _latest_snapshot(snapshots_root, channel)
        expected_hash = contract["source"][f"{channel}_content_sha256"]
        if latest["content_sha256"] != expected_hash:
            raise AssertionError(
                f"{channel} content hash changed: {latest['content_sha256']} != {expected_hash}"
            )
        actual = project_snapshot(snapshot, wanted)
        expected = {key: wanted[key] for key in ("cards", "relics", "monsters")}
        if actual != expected:
            raise AssertionError(f"{channel} supported-content projection changed")
        verified.append(channel)
    return verified


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--snapshots-root", type=Path, default=DEFAULT_SNAPSHOTS)
    args = parser.parse_args()
    channels = verify(args.contract, args.snapshots_root)
    print(json.dumps({"verified_channels": channels, "contract": str(args.contract)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
