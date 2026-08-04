from __future__ import annotations

import argparse
import json

from sls2_combat_solver.native import solve_simulator, step_simulator
from sls2_combat_solver.oracle import HttpGameOracle
from sls2_combat_solver.solver import SolveLimits


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Recreate the parity fixture and differentially replay its native solution."
    )
    parser.add_argument("--base-url", default="http://127.0.0.1:17351")
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    parser.add_argument("--allow-live-mutation", action="store_true", required=True)
    args = parser.parse_args()
    oracle = HttpGameOracle(args.base_url, timeout_seconds=args.timeout_seconds)
    snapshot = oracle.debug_start_nibbit(
        allow_live_mutation=True,
        timeout_milliseconds=int(args.timeout_seconds * 1000),
    )
    scenario = {"oracle": {"type": "simulator"}, "initial_state": snapshot}
    solution = solve_simulator(
        scenario,
        SolveLimits(max_states=100_000, max_turns=50, timeout_seconds=30),
    )
    if not solution["won"] or not solution["complete"]:
        raise SystemExit(f"native solver did not produce a complete win: {solution}")

    live_state = snapshot
    for index, action_id in enumerate(solution["action_ids"], start=1):
        legal = oracle.legal_actions(live_state)
        action = next((candidate for candidate in legal if candidate.get("id") == action_id), None)
        if action is None:
            raise SystemExit(f"step {index}: native action {action_id!r} is not legal in the game")
        expected = step_simulator(
            {"oracle": {"type": "simulator"}, "initial_state": live_state}, action
        )["state"]
        trace = oracle.live_trace_step(
            action,
            allow_live_mutation=True,
            timeout_milliseconds=int(args.timeout_seconds * 1000),
        )
        actual = trace["after"]
        if _semantic_state(actual) != _semantic_state(expected):
            print(
                json.dumps(
                    {
                        "step": index,
                        "action": action,
                        "expected": _semantic_state(expected),
                        "actual": _semantic_state(actual),
                    },
                    indent=2,
                    sort_keys=True,
                )
            )
            raise SystemExit(f"step {index}: native state diverged from the game")
        live_state = actual
        print(
            f"{index:02d} {action_id}: hp={actual['player']['hp']} "
            f"enemy_hp={[enemy['hp'] for enemy in actual['enemies']]}"
        )

    print(
        json.dumps(
            {
                "ok": True,
                "steps": len(solution["action_ids"]),
                "hp_loss": solution["hp_loss"],
                "final_hp": solution["final_hp"],
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 0


def _semantic_state(state: dict) -> dict:
    return {
        key: state[key]
        for key in (
            "rng",
            "combat",
            "decision",
            "player",
            "enemies",
            "hand",
            "draw_pile",
            "discard_pile",
            "exhaust_pile",
            "play_pile",
            "metrics",
        )
    }


if __name__ == "__main__":
    raise SystemExit(main())
