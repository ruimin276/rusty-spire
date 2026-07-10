from __future__ import annotations

import argparse
import json
import sys

from .compare import compare_scenarios
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
        elif args.command == "live-step":
            payload = _live_step_command(args)
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


if __name__ == "__main__":
    raise SystemExit(main())
