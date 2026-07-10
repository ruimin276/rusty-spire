from __future__ import annotations

import unittest

from sls2_combat_solver.oracle import MockOracle
from sls2_combat_solver.solver import SolveLimits, action_summary, solve_scenario


class SolverTests(unittest.TestCase):
    def test_first_popped_win_has_minimum_hp_loss(self) -> None:
        scenario = _scenario(
            states={
                "start": _state("start", hp=70, enemy_hp=10),
                "bad_win": _state("bad_win", hp=65, enemy_hp=0, won=True),
                "setup": _state("setup", hp=70, enemy_hp=5),
                "good_win": _state("good_win", hp=70, enemy_hp=0, won=True),
            },
            actions={
                "start": [_action("take_hit"), _action("setup")],
                "setup": [_action("finish")],
                "bad_win": [],
                "good_win": [],
            },
            transitions={
                "start": {"take_hit": "bad_win", "setup": "setup"},
                "setup": {"finish": "good_win"},
                "bad_win": {},
                "good_win": {},
            },
        )

        result = solve_scenario(scenario, MockOracle(scenario))

        self.assertTrue(result.won)
        self.assertEqual(result.hp_loss, 0)
        self.assertEqual(action_summary(result.actions), ["setup", "finish"])

    def test_equal_hp_loss_prefers_power_played(self) -> None:
        scenario = _scenario(
            states={
                "start": _state("start", hp=70, enemy_hp=10),
                "power_win": _state("power_win", hp=70, enemy_hp=0, won=True, powers=1),
                "plain_win": _state("plain_win", hp=70, enemy_hp=0, won=True),
            },
            actions={
                "start": [_action("plain"), _action("power")],
                "power_win": [],
                "plain_win": [],
            },
            transitions={
                "start": {"plain": "plain_win", "power": "power_win"},
                "power_win": {},
                "plain_win": {},
            },
        )

        result = solve_scenario(scenario, MockOracle(scenario))

        self.assertEqual(result.hp_loss, 0)
        self.assertEqual(action_summary(result.actions), ["power"])

    def test_hp_loss_beats_power_tie_breaker(self) -> None:
        scenario = _scenario(
            states={
                "start": _state("start", hp=70, enemy_hp=10),
                "power_loss_win": _state("power_loss_win", hp=69, enemy_hp=0, won=True, powers=1),
                "clean_win": _state("clean_win", hp=70, enemy_hp=0, won=True),
            },
            actions={
                "start": [_action("power_loss"), _action("clean")],
                "power_loss_win": [],
                "clean_win": [],
            },
            transitions={
                "start": {"power_loss": "power_loss_win", "clean": "clean_win"},
                "power_loss_win": {},
                "clean_win": {},
            },
        )

        result = solve_scenario(scenario, MockOracle(scenario))

        self.assertEqual(result.hp_loss, 0)
        self.assertEqual(action_summary(result.actions), ["clean"])

    def test_repeated_state_hashes_are_deduplicated(self) -> None:
        scenario = _scenario(
            states={
                "start": _state("start", hp=70, enemy_hp=10),
                "same": _state("same", hp=70, enemy_hp=5),
                "win": _state("win", hp=70, enemy_hp=0, won=True),
            },
            actions={
                "start": [_action("left"), _action("right")],
                "same": [_action("finish")],
                "win": [],
            },
            transitions={
                "start": {"left": "same", "right": "same"},
                "same": {"finish": "win"},
                "win": {},
            },
        )

        result = solve_scenario(scenario, MockOracle(scenario))

        self.assertTrue(result.won)
        self.assertGreaterEqual(result.cache_hits, 1)

    def test_no_win_returns_complete_failure(self) -> None:
        scenario = _scenario(
            states={
                "start": _state("start", hp=70, enemy_hp=10),
                "lost": _state("lost", hp=0, enemy_hp=10, lost=True),
            },
            actions={"start": [_action("die")], "lost": []},
            transitions={"start": {"die": "lost"}, "lost": {}},
        )

        result = solve_scenario(scenario, MockOracle(scenario))

        self.assertFalse(result.won)
        self.assertTrue(result.complete)
        self.assertEqual(result.termination_reason, "no_winning_line")

    def test_state_limit_returns_incomplete(self) -> None:
        scenario = _scenario(
            states={"start": _state("start", hp=70, enemy_hp=10)},
            actions={"start": [_action("loop")]},
            transitions={"start": {"loop": "start"}},
        )

        result = solve_scenario(
            scenario,
            MockOracle(scenario),
            SolveLimits(max_states=0, timeout_seconds=10),
        )

        self.assertFalse(result.complete)
        self.assertEqual(result.termination_reason, "max_states")


def _scenario(states, actions, transitions):
    return {
        "oracle": {"type": "mock"},
        "initial_state": states["start"],
        "mock": {"states": states, "actions": actions, "transitions": transitions},
    }


def _state(
    state_id: str,
    hp: int,
    enemy_hp: int,
    won: bool = False,
    lost: bool = False,
    powers: int = 0,
) -> dict:
    return {
        "id": state_id,
        "player": {"hp": hp, "block": 0},
        "combat": {"won": won, "lost": lost, "turn": 1},
        "metrics": {"powers_played": powers},
        "enemies": [{"id": "enemy", "hp": enemy_hp}],
    }


def _action(action_id: str) -> dict:
    return {"id": action_id, "type": "card"}


if __name__ == "__main__":
    unittest.main()
