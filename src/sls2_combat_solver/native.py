from __future__ import annotations

import json

from .oracle import OracleError
from .schema import JsonDict
from .solver import SolveLimits


def solve_simulator(scenario: JsonDict, limits: SolveLimits) -> JsonDict:
    native = _load_native()
    result = native.solve_simulator(
        json.dumps(scenario, separators=(",", ":"), sort_keys=True),
        json.dumps(_limits_json(limits), separators=(",", ":")),
    )
    return _object_result(result)


def compare_simulators(
    baseline: JsonDict,
    candidate: JsonDict,
    limits: SolveLimits,
) -> JsonDict:
    native = _load_native()
    result = native.compare_simulators(
        json.dumps(baseline, separators=(",", ":"), sort_keys=True),
        json.dumps(candidate, separators=(",", ":"), sort_keys=True),
        json.dumps(_limits_json(limits), separators=(",", ":")),
    )
    return _object_result(result)


def validate_simulator(scenario: JsonDict) -> None:
    native = _load_native()
    native.validate_snapshot(json.dumps(scenario, separators=(",", ":"), sort_keys=True))


def step_simulator(scenario: JsonDict, action: JsonDict) -> JsonDict:
    native = _load_native()
    result = native.step_simulator(
        json.dumps(scenario, separators=(",", ":"), sort_keys=True),
        json.dumps(action, separators=(",", ":"), sort_keys=True),
    )
    return _object_result(result)


def prepare_simulator(scenario: JsonDict) -> JsonDict:
    native = _load_native()
    result = native.prepare_simulator(
        json.dumps(scenario, separators=(",", ":"), sort_keys=True)
    )
    return _object_result(result)


def _load_native():
    try:
        from . import _native
    except ImportError as error:
        raise OracleError(
            "The Rust simulator extension is not installed. Run `python3 -m pip install -e .` "
            "or `maturin develop`."
        ) from error
    return _native


def _limits_json(limits: SolveLimits) -> JsonDict:
    return {
        "max_states": limits.max_states,
        "max_turns": limits.max_turns,
        "timeout_seconds": limits.timeout_seconds,
    }


def _object_result(raw: str) -> JsonDict:
    result = json.loads(raw)
    if not isinstance(result, dict):
        raise OracleError("Rust simulator returned a non-object result")
    return result
