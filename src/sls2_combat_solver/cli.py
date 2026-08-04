from __future__ import annotations

import argparse
import json
import sys

from .compare import compare_scenarios
from .native import compare_simulators, solve_simulator, validate_simulator
from .oracle import HttpGameOracle, OracleError, oracle_from_scenario
from .schema import ScenarioError, load_json, validate_scenario, write_json
from .solver import SolveLimits, action_summary, solve_scenario


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Solve exact Slay the Spire 2 combat scenarios through a game oracle."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    solve = subparsers.add_parser("solve", help="Solve one combat scenario.")
    solve.add_argument("--scenario", required=True, help="Scenario JSON file.")
    solve.add_argument("--output", help="Write result JSON to this file.")
    _add_limits(solve)

    compare = subparsers.add_parser("compare", help="Compare two scenario variants.")
    compare.add_argument("--baseline", required=True, help="Baseline scenario JSON file.")
    compare.add_argument("--candidate", required=True, help="Candidate scenario JSON file.")
    compare.add_argument("--output", help="Write comparison JSON to this file.")
    _add_limits(compare)

    export = subparsers.add_parser("export", help="Export current combat state from HTTP oracle.")
    export.add_argument("--base-url", default="http://127.0.0.1:17351")
    export.add_argument("--timeout-seconds", type=float, default=30.0)
    export.add_argument("--output", required=True, help="Write scenario JSON to this file.")

    capture = subparsers.add_parser(
        "capture",
        help="Capture a self-contained simulator snapshot at a stable player decision.",
    )
    capture.add_argument("--base-url", default="http://127.0.0.1:17351")
    capture.add_argument("--timeout-seconds", type=float, default=30.0)
    capture.add_argument("--output", required=True, help="Write simulator scenario JSON to this file.")

    health = subparsers.add_parser("oracle-health", help="Check HTTP oracle bridge health.")
    health.add_argument("--base-url", default="http://127.0.0.1:17351")
    health.add_argument("--timeout-seconds", type=float, default=5.0)

    live_step = subparsers.add_parser(
        "live-step",
        help="Apply one action to the active live combat through the HTTP oracle.",
    )
    live_step.add_argument("--scenario", required=True, help="Scenario JSON file.")
    live_step.add_argument("--action-id", required=True, help="Legal action id to apply.")
    live_step.add_argument("--output", help="Write resulting live state JSON to this file.")
    live_step.add_argument("--timeout-seconds", type=float, default=30.0)
    live_step.add_argument(
        "--allow-live-mutation",
        action="store_true",
        help="Required acknowledgement that this mutates the active game combat.",
    )

    trace_step = subparsers.add_parser(
        "trace-step",
        help="Record one live game transition for simulator differential testing.",
    )
    trace_step.add_argument("--scenario", required=True, help="Captured simulator scenario JSON file.")
    trace_step.add_argument("--action-id", required=True, help="Legal action id to execute.")
    trace_step.add_argument("--output", required=True, help="Write transition trace JSON to this file.")
    trace_step.add_argument("--timeout-seconds", type=float, default=30.0)
    trace_step.add_argument(
        "--allow-live-mutation",
        action="store_true",
        help="Required acknowledgement that this mutates the active game combat.",
    )

    debug_nibbit = subparsers.add_parser(
        "debug-start-nibbit",
        help="Replace the active run with the deterministic bridge parity fixture.",
    )
    debug_nibbit.add_argument("--base-url", default="http://127.0.0.1:17351")
    debug_nibbit.add_argument("--timeout-seconds", type=float, default=30.0)
    debug_nibbit.add_argument("--output", required=True, help="Write the captured simulator scenario.")
    debug_nibbit.add_argument(
        "--allow-live-mutation",
        action="store_true",
        help="Required acknowledgement that this replaces the active run.",
    )

    checkpoint = subparsers.add_parser(
        "live-checkpoint",
        help="Capture the active combat as a live restore checkpoint.",
    )
    checkpoint.add_argument("--base-url", default="http://127.0.0.1:17351")
    checkpoint.add_argument("--timeout-seconds", type=float, default=30.0)
    checkpoint.add_argument("--output", help="Write checkpoint response JSON to this file.")
    checkpoint.add_argument(
        "--allow-live-mutation",
        action="store_true",
        help="Required acknowledgement that checkpoint restore will mutate the active game.",
    )

    restore = subparsers.add_parser(
        "live-restore-checkpoint",
        help="Restore the active run to the last live checkpoint.",
    )
    restore.add_argument("--base-url", default="http://127.0.0.1:17351")
    restore.add_argument("--timeout-seconds", type=float, default=30.0)
    restore.add_argument("--output", help="Write restored state JSON to this file.")
    restore.add_argument(
        "--allow-live-mutation",
        action="store_true",
        help="Required acknowledgement that this replaces the active run state.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        if args.command == "solve":
            payload = _solve_command(args)
        elif args.command == "compare":
            payload = _compare_command(args)
        elif args.command == "export":
            payload = _export_command(args)
        elif args.command == "capture":
            payload = _capture_command(args)
        elif args.command == "live-step":
            payload = _live_step_command(args)
        elif args.command == "trace-step":
            payload = _trace_step_command(args)
        elif args.command == "debug-start-nibbit":
            payload = _debug_start_nibbit_command(args)
        elif args.command == "live-checkpoint":
            payload = _live_checkpoint_command(args)
        elif args.command == "live-restore-checkpoint":
            payload = _live_restore_checkpoint_command(args)
        else:
            payload = _health_command(args)
    except (ScenarioError, OracleError, ValueError) as error:
        parser.error(str(error))
    if getattr(args, "output", None):
        write_json(args.output, payload)
    else:
        json.dump(payload, sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
    return 0


def _solve_command(args: argparse.Namespace) -> dict:
    scenario = load_json(args.scenario)
    validate_scenario(scenario)
    if _oracle_type(scenario) == "simulator":
        validate_simulator(scenario)
        return solve_simulator(scenario, _limits_from_args(args))
    oracle = oracle_from_scenario(scenario)
    _require_branchable_step(oracle)
    result = solve_scenario(scenario, oracle, _limits_from_args(args))
    payload = result.to_json()
    payload["action_ids"] = action_summary(result.actions)
    return payload


def _compare_command(args: argparse.Namespace) -> dict:
    baseline = load_json(args.baseline)
    candidate = load_json(args.candidate)
    validate_scenario(baseline)
    validate_scenario(candidate)
    baseline_type = _oracle_type(baseline)
    candidate_type = _oracle_type(candidate)
    if baseline_type == "simulator" or candidate_type == "simulator":
        if baseline_type != "simulator" or candidate_type != "simulator":
            raise OracleError("Simulator comparison requires both scenarios to use oracle.type=simulator")
        validate_simulator(baseline)
        validate_simulator(candidate)
        return compare_simulators(baseline, candidate, _limits_from_args(args))
    baseline_oracle = oracle_from_scenario(baseline)
    candidate_oracle = oracle_from_scenario(candidate)
    _require_branchable_step(baseline_oracle)
    _require_branchable_step(candidate_oracle)
    comparison = compare_scenarios(
        baseline["initial_state"],
        candidate["initial_state"],
        baseline_oracle,
        candidate_oracle,
        _limits_from_args(args),
    )
    payload = comparison.to_json()
    payload["baseline"]["action_ids"] = action_summary(comparison.baseline.actions)
    payload["candidate"]["action_ids"] = action_summary(comparison.candidate.actions)
    return payload


def _export_command(args: argparse.Namespace) -> dict:
    oracle = HttpGameOracle(args.base_url, timeout_seconds=args.timeout_seconds)
    return {
        "oracle": {
            "type": "http",
            "base_url": args.base_url,
            "timeout_seconds": args.timeout_seconds,
        },
        "initial_state": oracle.export_state(),
    }


def _capture_command(args: argparse.Namespace) -> dict:
    oracle = HttpGameOracle(args.base_url, timeout_seconds=args.timeout_seconds)
    return {
        "oracle": {"type": "simulator"},
        "capture": {"base_url": args.base_url},
        "initial_state": oracle.export_sim_snapshot(),
    }


def _live_step_command(args: argparse.Namespace) -> dict:
    if not args.allow_live_mutation:
        raise OracleError("live-step requires --allow-live-mutation")
    scenario = load_json(args.scenario)
    validate_scenario(scenario)
    oracle = oracle_from_scenario(scenario)
    if not isinstance(oracle, HttpGameOracle):
        raise OracleError("live-step requires an HTTP oracle scenario")
    actions = oracle.legal_actions(scenario["initial_state"])
    action = next((item for item in actions if item.get("id") == args.action_id), None)
    if action is None:
        available = ", ".join(str(item.get("id")) for item in actions)
        raise OracleError(f"Unknown live action id {args.action_id!r}. Available: {available}")
    state = oracle.live_step(
        action,
        allow_live_mutation=True,
        timeout_milliseconds=int(args.timeout_seconds * 1000),
    )
    return {"action": action, "state": state}


def _trace_step_command(args: argparse.Namespace) -> dict:
    if not args.allow_live_mutation:
        raise OracleError("trace-step requires --allow-live-mutation")
    scenario = load_json(args.scenario)
    validate_scenario(scenario)
    initial_state = scenario["initial_state"]
    if not isinstance(initial_state, dict):
        raise OracleError("trace-step requires an object initial_state")
    provenance = initial_state.get("provenance", {})
    base_url = "http://127.0.0.1:17351"
    if isinstance(scenario.get("capture"), dict):
        base_url = str(scenario["capture"].get("base_url", base_url))
    oracle = HttpGameOracle(base_url, timeout_seconds=args.timeout_seconds)
    actions = oracle.legal_actions(initial_state)
    action = next((item for item in actions if item.get("id") == args.action_id), None)
    if action is None:
        available = ", ".join(str(item.get("id")) for item in actions)
        raise OracleError(f"Unknown trace action id {args.action_id!r}. Available: {available}")
    trace = oracle.live_trace_step(
        action,
        allow_live_mutation=True,
        timeout_milliseconds=int(args.timeout_seconds * 1000),
    )
    trace["captured_provenance"] = provenance
    return trace


def _debug_start_nibbit_command(args: argparse.Namespace) -> dict:
    if not args.allow_live_mutation:
        raise OracleError("debug-start-nibbit requires --allow-live-mutation")
    oracle = HttpGameOracle(args.base_url, timeout_seconds=args.timeout_seconds)
    snapshot = oracle.debug_start_nibbit(
        allow_live_mutation=True,
        timeout_milliseconds=int(args.timeout_seconds * 1000),
    )
    return {
        "oracle": {"type": "simulator"},
        "capture": {"base_url": args.base_url},
        "initial_state": snapshot,
    }


def _live_checkpoint_command(args: argparse.Namespace) -> dict:
    if not args.allow_live_mutation:
        raise OracleError("live-checkpoint requires --allow-live-mutation")
    oracle = HttpGameOracle(args.base_url, timeout_seconds=args.timeout_seconds)
    return oracle.live_checkpoint(allow_live_mutation=True)


def _live_restore_checkpoint_command(args: argparse.Namespace) -> dict:
    if not args.allow_live_mutation:
        raise OracleError("live-restore-checkpoint requires --allow-live-mutation")
    oracle = HttpGameOracle(args.base_url, timeout_seconds=args.timeout_seconds)
    state = oracle.live_restore_checkpoint(
        allow_live_mutation=True,
        timeout_milliseconds=int(args.timeout_seconds * 1000),
    )
    return {"state": state}


def _health_command(args: argparse.Namespace) -> dict:
    oracle = HttpGameOracle(args.base_url, timeout_seconds=args.timeout_seconds)
    return oracle.health()


def _require_branchable_step(oracle: object) -> None:
    if not isinstance(oracle, HttpGameOracle):
        return
    health = oracle.health()
    capabilities = health.get("capabilities", {})
    if isinstance(capabilities, dict) and capabilities.get("branchable_step") is False:
        raise OracleError(
            "HTTP oracle reports branchable_step=false; solving requires a bridge "
            "with branchable /step support."
        )


def _add_limits(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--max-states", type=int, default=100_000)
    parser.add_argument("--max-turns", type=int, default=50)
    parser.add_argument("--timeout-seconds", type=float, default=60.0)


def _limits_from_args(args: argparse.Namespace) -> SolveLimits:
    return SolveLimits(
        max_states=args.max_states,
        max_turns=args.max_turns,
        timeout_seconds=args.timeout_seconds,
    )


def _oracle_type(scenario: dict) -> str:
    config = scenario.get("oracle", {})
    if not isinstance(config, dict):
        return ""
    return str(config.get("type", "http")).lower()


if __name__ == "__main__":
    raise SystemExit(main())
