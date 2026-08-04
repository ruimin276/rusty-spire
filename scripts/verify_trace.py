from __future__ import annotations

import argparse
import json
from pathlib import Path

from sls2_combat_solver.native import step_simulator


def main() -> int:
    parser = argparse.ArgumentParser(description="Compare a bridge trace with one native transition.")
    parser.add_argument("trace", type=Path)
    args = parser.parse_args()
    trace = json.loads(args.trace.read_text(encoding="utf-8"))
    scenario = {"oracle": {"type": "simulator"}, "initial_state": trace["before"]}
    simulated = step_simulator(scenario, trace["action"])["state"]
    expected = _semantic_state(trace["after"])
    actual = _semantic_state(simulated)
    if actual != expected:
        print(json.dumps({"expected": expected, "actual": actual}, indent=2, sort_keys=True))
        raise SystemExit("native transition differs from bridge trace")
    print(
        json.dumps(
            {
                "ok": True,
                "action": trace["action"]["id"],
                "before_game_checksum": trace.get("before_game_checksum"),
                "after_game_checksum": trace.get("after_game_checksum"),
                "rng_deltas": trace.get("rng_deltas", {}),
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


def _semantic_state(state: dict) -> dict:
    return {
        "rng": state["rng"],
        "combat": state["combat"],
        "decision": state["decision"],
        "player": state["player"],
        "enemies": state["enemies"],
        "hand": state["hand"],
        "draw_pile": state["draw_pile"],
        "discard_pile": state["discard_pile"],
        "exhaust_pile": state["exhaust_pile"],
        "play_pile": state["play_pile"],
        "metrics": state["metrics"],
    }


if __name__ == "__main__":
    raise SystemExit(main())
