from __future__ import annotations

import json
import urllib.error
import urllib.request
from typing import Protocol

from .schema import JsonDict, ScenarioError, action_id, canonical_json


class OracleError(RuntimeError):
    pass


class CombatOracle(Protocol):
    def export_state(self) -> JsonDict:
        ...

    def legal_actions(self, state: JsonDict) -> list[JsonDict]:
        ...

    def step(self, state: JsonDict, action: JsonDict) -> JsonDict:
        ...

    def state_hash(self, state: JsonDict) -> str:
        ...


class MockOracle:
    """Deterministic oracle for tests and scenario-format development."""

    def __init__(self, scenario: JsonDict) -> None:
        mock = scenario.get("mock")
        if not isinstance(mock, dict):
            raise ScenarioError("Mock oracle scenarios must include a mock object")
        self._states = _require_mapping(mock, "states")
        self._actions = _require_mapping(mock, "actions")
        self._transitions = _require_mapping(mock, "transitions")

    def export_state(self) -> JsonDict:
        return dict(self._states.get("start", {}))

    def legal_actions(self, state: JsonDict) -> list[JsonDict]:
        state_key = self.state_hash(state)
        actions = self._actions.get(state_key, [])
        if not isinstance(actions, list):
            raise OracleError(f"Mock actions for {state_key!r} must be a list")
        return [dict(action) for action in actions if isinstance(action, dict)]

    def step(self, state: JsonDict, action: JsonDict) -> JsonDict:
        state_key = self.state_hash(state)
        transitions = self._transitions.get(state_key, {})
        if not isinstance(transitions, dict):
            raise OracleError(f"Mock transitions for {state_key!r} must be an object")
        next_key = transitions.get(action_id(action))
        if next_key is None:
            raise OracleError(f"No mock transition for {state_key!r} / {action_id(action)!r}")
        next_state = self._states.get(str(next_key))
        if not isinstance(next_state, dict):
            raise OracleError(f"No mock state named {next_key!r}")
        return dict(next_state)

    def state_hash(self, state: JsonDict) -> str:
        if "id" in state:
            return str(state["id"])
        return canonical_json(state)


class HttpGameOracle:
    """HTTP bridge expected to be implemented by the STS2 combat oracle mod."""

    def __init__(self, base_url: str, timeout_seconds: float = 30.0) -> None:
        self.base_url = base_url.rstrip("/")
        self.timeout_seconds = timeout_seconds

    def export_state(self) -> JsonDict:
        response = self._post("/export_state", {})
        state = response.get("state", response)
        if not isinstance(state, dict):
            raise OracleError("Oracle /export_state response must include state object")
        return state

    def health(self) -> JsonDict:
        return self._post("/health", {})

    def legal_actions(self, state: JsonDict) -> list[JsonDict]:
        response = self._post("/legal_actions", {"state": state})
        actions = response.get("actions")
        if not isinstance(actions, list):
            raise OracleError("Oracle /legal_actions response must include actions list")
        return [dict(action) for action in actions if isinstance(action, dict)]

    def step(self, state: JsonDict, action: JsonDict) -> JsonDict:
        response = self._post("/step", {"state": state, "action": action})
        next_state = response.get("state")
        if not isinstance(next_state, dict):
            raise OracleError("Oracle /step response must include state object")
        return next_state

    def live_step(
        self,
        action: JsonDict,
        *,
        allow_live_mutation: bool = False,
        timeout_milliseconds: int = 30_000,
    ) -> JsonDict:
        response = self._post(
            "/live_step",
            {
                "action": action,
                "allow_live_mutation": allow_live_mutation,
                "timeout_milliseconds": timeout_milliseconds,
            },
        )
        next_state = response.get("state")
        if not isinstance(next_state, dict):
            raise OracleError("Oracle /live_step response must include state object")
        return next_state

    def live_checkpoint(self, *, allow_live_mutation: bool = False) -> JsonDict:
        response = self._post(
            "/live_checkpoint",
            {"allow_live_mutation": allow_live_mutation},
        )
        if not isinstance(response, dict):
            raise OracleError("Oracle /live_checkpoint response must be an object")
        return response

    def live_restore_checkpoint(
        self,
        *,
        allow_live_mutation: bool = False,
        timeout_milliseconds: int = 30_000,
    ) -> JsonDict:
        response = self._post(
            "/live_restore_checkpoint",
            {
                "allow_live_mutation": allow_live_mutation,
                "timeout_milliseconds": timeout_milliseconds,
            },
        )
        next_state = response.get("state")
        if not isinstance(next_state, dict):
            raise OracleError("Oracle /live_restore_checkpoint response must include state object")
        return next_state

    def state_hash(self, state: JsonDict) -> str:
        response = self._post("/state_hash", {"state": state})
        state_hash = response.get("state_hash")
        if not isinstance(state_hash, str):
            raise OracleError("Oracle /state_hash response must include state_hash string")
        return state_hash

    def _post(self, path: str, payload: JsonDict) -> JsonDict:
        data = json.dumps(payload).encode("utf-8")
        request = urllib.request.Request(
            f"{self.base_url}{path}",
            data=data,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(request, timeout=self.timeout_seconds) as response:
                body = response.read().decode("utf-8")
        except urllib.error.HTTPError as error:
            try:
                body = error.read().decode("utf-8", errors="replace")
            finally:
                error.close()
            message = _error_message(body) or error.reason
            raise OracleError(
                f"Oracle request failed for {path}: HTTP {error.code}: {message}"
            ) from error
        except urllib.error.URLError as error:
            raise OracleError(f"Oracle request failed for {path}: {error}") from error
        parsed = json.loads(body)
        if not isinstance(parsed, dict):
            raise OracleError(f"Oracle response for {path} must be an object")
        return parsed


def oracle_from_scenario(scenario: JsonDict) -> CombatOracle:
    oracle_config = scenario.get("oracle", {"type": "http"})
    if not isinstance(oracle_config, dict):
        raise ScenarioError("Scenario oracle must be an object")
    oracle_type = str(oracle_config.get("type", "http")).lower()
    if oracle_type == "mock":
        return MockOracle(scenario)
    if oracle_type == "http":
        base_url = oracle_config.get("base_url")
        if not isinstance(base_url, str) or not base_url:
            raise ScenarioError("HTTP oracle requires oracle.base_url")
        timeout = float(oracle_config.get("timeout_seconds", 30.0))
        return HttpGameOracle(base_url, timeout_seconds=timeout)
    raise ScenarioError(f"Unsupported oracle type: {oracle_type}")


def _error_message(body: str) -> str | None:
    try:
        payload = json.loads(body)
    except json.JSONDecodeError:
        return body.strip() or None
    if isinstance(payload, dict) and isinstance(payload.get("error"), str):
        return payload["error"]
    return body.strip() or None


def _require_mapping(parent: JsonDict, key: str) -> JsonDict:
    value = parent.get(key)
    if not isinstance(value, dict):
        raise ScenarioError(f"mock.{key} must be an object")
    return value
