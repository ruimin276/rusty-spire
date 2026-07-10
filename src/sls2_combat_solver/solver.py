from __future__ import annotations

import heapq
import itertools
import time
from dataclasses import asdict, dataclass

from .oracle import CombatOracle, OracleError
from .schema import (
    JsonDict,
    action_id,
    enemy_hp_total,
    is_lost,
    is_potion_action,
    is_won,
    player_block,
    player_hp,
    powers_played,
    turn_number,
)


@dataclass(frozen=True)
class SolveLimits:
    max_states: int = 100_000
    max_turns: int = 50
    timeout_seconds: float = 60.0


@dataclass(frozen=True)
class TraceStep:
    action: JsonDict
    state_hash: str
    hp_loss: int


@dataclass(frozen=True)
class SolveResult:
    won: bool
    complete: bool
    hp_loss: int | None
    final_hp: int | None
    actions: list[TraceStep]
    explored_states: int
    cache_hits: int
    runtime_seconds: float
    termination_reason: str

    def to_json(self) -> JsonDict:
        data = asdict(self)
        data["actions"] = [
            {"action": step.action, "state_hash": step.state_hash, "hp_loss": step.hp_loss}
            for step in self.actions
        ]
        return data


@dataclass(frozen=True)
class _Node:
    state: JsonDict
    state_hash: str
    hp_loss: int
    actions: tuple[TraceStep, ...]


def solve_combat(
    initial_state: JsonDict,
    oracle: CombatOracle,
    limits: SolveLimits | None = None,
) -> SolveResult:
    limits = limits or SolveLimits()
    start_time = time.monotonic()
    starting_hp = player_hp(initial_state)
    initial_hash = oracle.state_hash(initial_state)
    initial_node = _Node(initial_state, initial_hash, 0, ())
    frontier: list[tuple[tuple[int, int, int, int, int, int], _Node]] = []
    sequence = itertools.count()
    heapq.heappush(frontier, (_priority(initial_node, next(sequence)), initial_node))
    best_loss_by_hash = {initial_hash: 0}
    explored_states = 0
    cache_hits = 0

    while frontier:
        elapsed = time.monotonic() - start_time
        if elapsed > limits.timeout_seconds:
            return _incomplete_result(
                explored_states, cache_hits, elapsed, "timeout"
            )

        _, node = heapq.heappop(frontier)
        if node.hp_loss != best_loss_by_hash.get(node.state_hash):
            cache_hits += 1
            continue

        if is_won(node.state):
            return SolveResult(
                won=True,
                complete=True,
                hp_loss=node.hp_loss,
                final_hp=player_hp(node.state),
                actions=list(node.actions),
                explored_states=explored_states,
                cache_hits=cache_hits,
                runtime_seconds=time.monotonic() - start_time,
                termination_reason="optimal_win",
            )

        if is_lost(node.state):
            explored_states += 1
            continue
        if explored_states >= limits.max_states:
            return _incomplete_result(
                explored_states, cache_hits, time.monotonic() - start_time, "max_states"
            )
        if turn_number(node.state) > limits.max_turns:
            explored_states += 1
            continue

        explored_states += 1
        for action in oracle.legal_actions(node.state):
            if is_potion_action(action):
                continue
            next_state = oracle.step(node.state, action)
            next_hash = oracle.state_hash(next_state)
            next_loss = max(node.hp_loss, starting_hp - player_hp(next_state))
            if next_loss >= best_loss_by_hash.get(next_hash, 10**9):
                cache_hits += 1
                continue
            best_loss_by_hash[next_hash] = next_loss
            step = TraceStep(action=action, state_hash=next_hash, hp_loss=next_loss)
            child = _Node(
                state=next_state,
                state_hash=next_hash,
                hp_loss=next_loss,
                actions=(*node.actions, step),
            )
            heapq.heappush(frontier, (_priority(child, next(sequence)), child))

    return SolveResult(
        won=False,
        complete=True,
        hp_loss=None,
        final_hp=None,
        actions=[],
        explored_states=explored_states,
        cache_hits=cache_hits,
        runtime_seconds=time.monotonic() - start_time,
        termination_reason="no_winning_line",
    )


def _priority(node: _Node, sequence: int) -> tuple[int, int, int, int, int, int]:
    return (
        node.hp_loss,
        -powers_played(node.state),
        enemy_hp_total(node.state),
        -player_block(node.state),
        len(node.actions),
        sequence,
    )


def _incomplete_result(
    explored_states: int,
    cache_hits: int,
    runtime_seconds: float,
    reason: str,
) -> SolveResult:
    return SolveResult(
        won=False,
        complete=False,
        hp_loss=None,
        final_hp=None,
        actions=[],
        explored_states=explored_states,
        cache_hits=cache_hits,
        runtime_seconds=runtime_seconds,
        termination_reason=reason,
    )


def solve_scenario(
    scenario: JsonDict,
    oracle: CombatOracle,
    limits: SolveLimits | None = None,
) -> SolveResult:
    try:
        return solve_combat(scenario["initial_state"], oracle, limits)
    except OracleError:
        raise
    except KeyError as error:
        raise ValueError("Scenario is missing initial_state") from error


def action_summary(actions: list[TraceStep]) -> list[str]:
    return [action_id(step.action) for step in actions]
