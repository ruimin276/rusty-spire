from __future__ import annotations

import json
from pathlib import Path
from typing import Any

JsonDict = dict[str, Any]


class ScenarioError(ValueError):
    pass


def load_json(path: str | Path) -> JsonDict:
    with Path(path).open(encoding="utf-8") as file:
        data = json.load(file)
    if not isinstance(data, dict):
        raise ScenarioError(f"{path} must contain a JSON object")
    return data


def write_json(path: str | Path, data: JsonDict) -> None:
    with Path(path).open("w", encoding="utf-8") as file:
        json.dump(data, file, indent=2, sort_keys=True)
        file.write("\n")


def canonical_json(data: Any) -> str:
    return json.dumps(data, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def validate_scenario(scenario: JsonDict) -> None:
    if "initial_state" not in scenario:
        raise ScenarioError("Scenario is missing initial_state")
    if not isinstance(scenario["initial_state"], dict):
        raise ScenarioError("Scenario initial_state must be an object")
    if "oracle" in scenario and not isinstance(scenario["oracle"], dict):
        raise ScenarioError("Scenario oracle must be an object")
    player_hp(scenario["initial_state"])


def player_hp(state: JsonDict) -> int:
    try:
        return int(state["player"]["hp"])
    except (KeyError, TypeError, ValueError) as error:
        raise ScenarioError("State must include integer player.hp") from error


def player_block(state: JsonDict) -> int:
    player = state.get("player", {})
    if not isinstance(player, dict):
        return 0
    return int(player.get("block", 0))


def is_won(state: JsonDict) -> bool:
    combat = state.get("combat", {})
    if isinstance(combat, dict) and bool(combat.get("won", False)):
        return True
    enemies = state.get("enemies", [])
    if isinstance(enemies, list) and enemies:
        return all(int(enemy.get("hp", 0)) <= 0 for enemy in enemies if isinstance(enemy, dict))
    return False


def is_lost(state: JsonDict) -> bool:
    combat = state.get("combat", {})
    if isinstance(combat, dict) and bool(combat.get("lost", False)):
        return True
    return player_hp(state) <= 0


def turn_number(state: JsonDict) -> int:
    combat = state.get("combat", {})
    if not isinstance(combat, dict):
        return 0
    return int(combat.get("turn", 0))


def powers_played(state: JsonDict) -> int:
    metrics = state.get("metrics", {})
    if isinstance(metrics, dict) and "powers_played" in metrics:
        return int(metrics["powers_played"])
    combat = state.get("combat", {})
    if isinstance(combat, dict) and "powers_played" in combat:
        return int(combat["powers_played"])
    return 0


def enemy_hp_total(state: JsonDict) -> int:
    enemies = state.get("enemies", [])
    if not isinstance(enemies, list):
        return 0
    total = 0
    for enemy in enemies:
        if isinstance(enemy, dict):
            total += max(0, int(enemy.get("hp", 0)))
    return total


def is_potion_action(action: JsonDict) -> bool:
    return str(action.get("type", "")).lower() == "potion"


def action_id(action: JsonDict) -> str:
    if "id" not in action:
        raise ScenarioError(f"Action is missing id: {action!r}")
    return str(action["id"])
