from __future__ import annotations

import argparse
import json
from pathlib import Path

from sls2_combat_solver.native import prepare_simulator, solve_simulator
from sls2_combat_solver.solver import SolveLimits


ENEMIES = {
    "nibbit": ("MONSTER.NIBBIT", 44, "BUTT_MOVE"),
    "fuzzy_wurm_crawler": (
        "MONSTER.FUZZY_WURM_CRAWLER",
        56,
        "FIRST_ACID_GOOP_MOVE",
    ),
    "shrinker_beetle": ("MONSTER.SHRINKER_BEETLE", 39, "SHRINKER_MOVE"),
}


def card(instance_id: int, model_id: str, cost: int) -> dict[str, object]:
    return {
        "instance_id": str(instance_id),
        "model_id": model_id,
        "upgrade_level": 0,
        "cost": cost,
        "cost_for_turn": cost,
    }


def unshuffled_scenario(enemy_name: str, shuffle_seed: int) -> dict[str, object]:
    model_id, hp, opening_move = ENEMIES[enemy_name]
    deck = [
        *[card(index, "CARD.STRIKE_SILENT", 1) for index in range(5)],
        *[card(index, "CARD.DEFEND_SILENT", 1) for index in range(5, 10)],
        card(10, "CARD.NEUTRALIZE", 0),
        card(11, "CARD.SURVIVOR", 1),
    ]
    return {
        "oracle": {"type": "simulator"},
        "initial_state": {
            "snapshot_schema": 2,
            "provenance": {
                "game_version": "source-reference-2026-08-04",
                "game_commit": "offline-source-matrix",
                "assembly_sha256": "unavailable-local-install",
                "content_revision": "base",
                "modded_gameplay": False,
            },
            "rng": {
                "algorithm": "xoshiro256_star_star_v1",
                "run_seed": f"SILENT-WEAK-{shuffle_seed}",
                "streams": {"shuffle": {"seed": shuffle_seed, "counter": 0}},
            },
            "combat": {
                "won": False,
                "lost": False,
                "turn": 1,
                "current_side": "Player",
                "ascension_level": 0,
            },
            "decision": {"kind": "player_action"},
            "player": {
                "combat_id": "0",
                "model_id": "CHARACTER.SILENT",
                "hp": 70,
                "max_hp": 70,
                "block": 0,
                "energy": 3,
                "max_energy": 3,
                "powers": [],
                "relics": [{"model_id": "RELIC.RING_OF_THE_SNAKE"}],
                "potions": [],
            },
            "enemies": [
                {
                    "combat_id": "1",
                    "model_id": model_id,
                    "hp": hp,
                    "max_hp": hp,
                    "block": 0,
                    "powers": [],
                    "ai": {
                        "current_move": opening_move,
                        "move_history": [],
                        "is_front": False,
                        "is_alone": True,
                        "tough_enemies": False,
                        "deadly_enemies": False,
                    },
                }
            ],
            "hand": [],
            "draw_pile": deck,
            "discard_pile": [],
            "exhaust_pile": [],
            "play_pile": [],
            "metrics": {"powers_played": 0},
        },
    }


def run_matrix(seeds: list[int], max_states: int, timeout_seconds: float) -> dict[str, object]:
    limits = SolveLimits(
        max_states=max_states,
        max_turns=50,
        timeout_seconds=timeout_seconds,
    )
    cases = []
    for enemy_name in ENEMIES:
        for seed in seeds:
            scenario = prepare_simulator(unshuffled_scenario(enemy_name, seed))
            result = solve_simulator(scenario, limits)
            cases.append(
                {
                    "enemy": enemy_name,
                    "shuffle_seed": seed,
                    "opening_hand": [
                        card_state["model_id"]
                        for card_state in scenario["initial_state"]["hand"]
                    ],
                    "won": result["won"],
                    "complete": result["complete"],
                    "hp_loss": result["hp_loss"],
                    "final_hp": result["final_hp"],
                    "enemy_turns": sum(
                        step["action"].get("id") == "end_turn"
                        for step in result["actions"]
                    ),
                    "actions": len(result["action_ids"]),
                    "explored_states": result["explored_states"],
                    "runtime_seconds": result["runtime_seconds"],
                    "termination_reason": result["termination_reason"],
                }
            )
    return {
        "evidence": "offline_source_backed_not_live_differential",
        "assumptions": {
            "act": "Overgrowth easy pool",
            "ascension": 0,
            "enemy_hp": "fixed midpoint of the published A0 range",
            "rng": "shuffle seeds are supplied directly to the verified xoshiro adapter",
            "live_game_available": False,
        },
        "sources": [
            "https://slaythespire.wiki.gg/wiki/Slay_the_Spire_2:Cards",
            "https://slaythespire.wiki.gg/wiki/Slay_the_Spire_2:Neutralize",
            "https://slaythespire.wiki.gg/wiki/Slay_the_Spire_2:Fuzzy_Wurm_Crawler",
            "https://slaythespire.wiki.gg/wiki/Slay_the_Spire_2:Shrinker_Beetle",
            "https://sts2.wiki/encounters/",
        ],
        "seeds": seeds,
        "cases": cases,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--seeds", default="1,7,42")
    parser.add_argument("--max-states", type=int, default=1_000_000)
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    seeds = [int(value) for value in args.seeds.split(",")]
    report = run_matrix(seeds, args.max_states, args.timeout_seconds)
    rendered = json.dumps(report, indent=2)
    if args.output:
        args.output.write_text(rendered + "\n", encoding="utf-8")
    print(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
