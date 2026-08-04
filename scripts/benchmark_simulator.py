from __future__ import annotations

import argparse
import json
import time
from pathlib import Path

from sls2_combat_solver.native import solve_simulator
from sls2_combat_solver.solver import SolveLimits


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser(description="Run the native simulator performance gate.")
    parser.add_argument("--max-seconds", type=float, default=5.0)
    args = parser.parse_args()
    scenario = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text(encoding="utf-8"))
    # Prevent this benchmark-only state from reaching a terminal before the
    # expansion budget. Mechanics and branching remain the real native path.
    scenario["initial_state"]["player"]["hp"] = 1_000_000
    scenario["initial_state"]["player"]["max_hp"] = 1_000_000
    scenario["initial_state"]["enemies"][0]["hp"] = 1_000_000
    scenario["initial_state"]["enemies"][0]["max_hp"] = 1_000_000
    limits = SolveLimits(max_states=100_000, max_turns=1_000, timeout_seconds=max(30.0, args.max_seconds))

    started = time.perf_counter()
    result = solve_simulator(scenario, limits)
    wall_seconds = time.perf_counter() - started
    report = {
        "explored_states": result["explored_states"],
        "native_runtime_seconds": result["runtime_seconds"],
        "wall_runtime_seconds": wall_seconds,
        "termination_reason": result["termination_reason"],
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    if result["explored_states"] != 100_000:
        raise SystemExit("benchmark fixture did not exercise exactly 100,000 states")
    if wall_seconds >= args.max_seconds:
        raise SystemExit(
            f"performance gate failed: {wall_seconds:.3f}s is not below {args.max_seconds:.3f}s"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
