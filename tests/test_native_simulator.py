from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from sls2_combat_solver.cli import main
from sls2_combat_solver.native import compare_simulators, solve_simulator, step_simulator
from sls2_combat_solver.solver import SolveLimits
from scripts.run_silent_weak_matrix import run_matrix


ROOT = Path(__file__).resolve().parents[1]


class NativeSimulatorTests(unittest.TestCase):
    def test_silent_single_enemy_weak_pool_across_seeds(self) -> None:
        report = run_matrix([1, 2, 4, 7, 17, 42], max_states=100_000, timeout_seconds=5)
        cases = report["cases"]

        self.assertEqual(len(cases), 18)
        self.assertEqual(
            {case["enemy"] for case in cases},
            {"nibbit", "fuzzy_wurm_crawler", "shrinker_beetle"},
        )
        self.assertTrue(all(case["won"] and case["complete"] for case in cases))
        self.assertTrue(any(case["hp_loss"] > 0 for case in cases))
        self.assertGreater(
            len({tuple(case["opening_hand"]) for case in cases}),
            1,
        )
        expected = json.loads(
            (ROOT / "tests" / "fixtures" / "silent_weak_seed_matrix.json").read_text()
        )
        expected_outcomes = {
            (case["enemy"], case["shuffle_seed"]): (
                case["hp_loss"],
                case["final_hp"],
                case["enemy_turns"],
            )
            for case in expected["cases"]
        }
        self.assertEqual(
            {
                (case["enemy"], case["shuffle_seed"]): (
                    case["hp_loss"],
                    case["final_hp"],
                    case["enemy_turns"],
                )
                for case in cases
            },
            expected_outcomes,
        )

    def test_native_solution_matches_full_live_replay(self) -> None:
        scenario = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text())
        golden = json.loads(
            (ROOT / "tests" / "fixtures" / "v108_solution_replay.json").read_text()
        )
        result = solve_simulator(
            scenario,
            SolveLimits(max_states=100_000, max_turns=50, timeout_seconds=30),
        )
        expected = golden["expected"]

        self.assertEqual(result["won"], expected["won"])
        self.assertEqual(result["complete"], expected["complete"])
        self.assertEqual(result["hp_loss"], expected["hp_loss"])
        self.assertEqual(result["final_hp"], expected["final_hp"])
        self.assertEqual(result["action_ids"], golden["action_ids"])
        self.assertEqual(len(result["action_ids"]), expected["steps"])

    def test_native_comparison_keeps_search_in_rust(self) -> None:
        baseline = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text())
        candidate = json.loads(json.dumps(baseline))
        for scenario, enemy_hp in ((baseline, 7), (candidate, 6)):
            state = scenario["initial_state"]
            strike = next(
                card for card in state["hand"] if card["model_id"] == "CARD.STRIKE_IRONCLAD"
            )
            state["hand"] = [strike]
            state["draw_pile"] = []
            state["discard_pile"] = []
            state["enemies"][0]["hp"] = enemy_hp
            state["enemies"][0]["max_hp"] = enemy_hp

        result = compare_simulators(
            baseline,
            candidate,
            SolveLimits(max_states=1_000, max_turns=5, timeout_seconds=5),
        )

        self.assertEqual(result["better"], "candidate")
        self.assertEqual(result["candidate"]["hp_loss"], 0)
        self.assertGreater(result["baseline"]["hp_loss"], 0)

    def test_bridge_captured_hiss_transition_matches_native_state(self) -> None:
        scenario = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text())
        golden = json.loads(
            (ROOT / "tests" / "fixtures" / "v108_hiss_transition.json").read_text()
        )
        state = scenario["initial_state"]
        for action in [*golden["prefix_actions"], golden["action"]]:
            state = step_simulator(
                {"oracle": {"type": "simulator"}, "initial_state": state}, action
            )["state"]
        expected = golden["expected"]

        self.assertEqual(state["combat"]["turn"], expected["turn"])
        self.assertEqual(state["player"]["hp"], expected["player_hp"])
        self.assertEqual(state["enemies"][0]["hp"], expected["enemy_hp"])
        self.assertEqual(state["enemies"][0]["ai"]["current_move"], expected["enemy_move"])
        self.assertEqual(state["enemies"][0]["powers"], expected["enemy_powers"])
        self.assertEqual(
            [card["instance_id"] for card in state["hand"]], expected["hand_instance_ids"]
        )

    def test_bridge_captured_bash_transition_matches_native_state(self) -> None:
        scenario = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text())
        golden = json.loads(
            (ROOT / "tests" / "fixtures" / "v108_bash_transition.json").read_text()
        )
        end_turn = {"id": "end_turn", "type": "end_turn"}
        after_first = step_simulator(scenario, end_turn)["state"]
        after_second = step_simulator(
            {"oracle": {"type": "simulator"}, "initial_state": after_first}, end_turn
        )["state"]
        after = step_simulator(
            {"oracle": {"type": "simulator"}, "initial_state": after_second},
            golden["action"],
        )["state"]
        expected = golden["expected"]

        self.assertEqual(after["player"]["energy"], expected["player_energy"])
        self.assertEqual(after["enemies"][0]["hp"], expected["enemy_hp"])
        self.assertEqual(after["enemies"][0]["block"], expected["enemy_block"])
        self.assertEqual(after["enemies"][0]["powers"], expected["enemy_powers"])
        self.assertEqual(
            [card["instance_id"] for card in after["hand"]], expected["hand_instance_ids"]
        )
        self.assertEqual(
            [card["instance_id"] for card in after["discard_pile"]],
            expected["discard_instance_ids"],
        )

    def test_bridge_captured_reshuffle_preserves_exact_rng_order(self) -> None:
        scenario = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text())
        golden = json.loads(
            (ROOT / "tests" / "fixtures" / "v108_reshuffle_transition.json").read_text()
        )
        end_turn = golden["action"]
        after_first = step_simulator(scenario, end_turn)["state"]
        second_scenario = {"oracle": {"type": "simulator"}, "initial_state": after_first}
        after = step_simulator(second_scenario, end_turn)["state"]
        expected = golden["expected"]

        self.assertEqual(after["combat"]["turn"], expected["turn"])
        self.assertEqual(after["player"]["hp"], expected["player_hp"])
        self.assertEqual(after["enemies"][0]["block"], expected["enemy_block"])
        self.assertEqual(after["enemies"][0]["ai"]["current_move"], expected["enemy_move"])
        for pile in ("hand", "draw_pile", "discard_pile", "exhaust_pile"):
            key = pile.removesuffix("_pile") + "_instance_ids"
            self.assertEqual([card["instance_id"] for card in after[pile]], expected[key])
        self.assertEqual(
            after_first["rng"]["streams"]["shuffle"]["counter"],
            expected["shuffle_counter_before"],
        )
        self.assertEqual(
            after["rng"]["streams"]["shuffle"]["counter"],
            expected["shuffle_counter_after"],
        )

    def test_bridge_captured_end_turn_transition_matches_native_state(self) -> None:
        scenario = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text())
        golden = json.loads(
            (ROOT / "tests" / "fixtures" / "v108_end_turn_transition.json").read_text()
        )

        after = step_simulator(scenario, golden["action"])["state"]
        expected = golden["expected"]

        self.assertEqual(after["combat"]["turn"], expected["turn"])
        self.assertEqual(after["player"]["hp"], expected["player_hp"])
        self.assertEqual(after["player"]["block"], expected["player_block"])
        self.assertEqual(after["player"]["energy"], expected["player_energy"])
        self.assertEqual(after["enemies"][0]["hp"], expected["enemy_hp"])
        self.assertEqual(after["enemies"][0]["ai"]["current_move"], expected["enemy_move"])
        self.assertEqual(
            after["enemies"][0]["ai"]["move_history"], expected["enemy_move_history"]
        )
        for pile in ("hand", "draw_pile", "discard_pile", "exhaust_pile"):
            key = pile.removesuffix("_pile") + "_instance_ids"
            self.assertEqual([card["instance_id"] for card in after[pile]], expected[key])
        before_shuffle = scenario["initial_state"]["rng"]["streams"]["shuffle"]["counter"]
        self.assertEqual(
            after["rng"]["streams"]["shuffle"]["counter"] - before_shuffle,
            expected["shuffle_counter_delta"],
        )

    def test_bridge_captured_defend_transition_matches_native_state(self) -> None:
        scenario = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text())
        golden = json.loads(
            (ROOT / "tests" / "fixtures" / "v108_defend_transition.json").read_text()
        )

        after = step_simulator(scenario, golden["action"])["state"]
        expected = golden["expected"]

        self.assertEqual(after["player"]["hp"], expected["player_hp"])
        self.assertEqual(after["player"]["block"], expected["player_block"])
        self.assertEqual(after["player"]["energy"], expected["player_energy"])
        self.assertEqual(after["enemies"][0]["hp"], expected["enemy_hp"])
        self.assertEqual(
            [card["instance_id"] for card in after["hand"]],
            expected["hand_instance_ids"],
        )
        self.assertEqual(
            [card["instance_id"] for card in after["discard_pile"]],
            expected["discard_instance_ids"],
        )
        for stream, delta in expected["rng_deltas"].items():
            before_counter = scenario["initial_state"]["rng"]["streams"][stream]["counter"]
            self.assertEqual(after["rng"]["streams"][stream]["counter"] - before_counter, delta)

    def test_cli_solves_simulator_scenario_through_rust(self) -> None:
        scenario = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text())
        state = scenario["initial_state"]
        state["enemies"][0]["hp"] = 6
        state["enemies"][0]["max_hp"] = 6
        strike = next(card for card in state["hand"] if card["model_id"] == "CARD.STRIKE_IRONCLAD")
        state["hand"] = [strike]
        state["draw_pile"] = []

        with tempfile.TemporaryDirectory() as directory:
            scenario_path = Path(directory) / "scenario.json"
            output_path = Path(directory) / "result.json"
            scenario_path.write_text(json.dumps(scenario), encoding="utf-8")

            exit_code = main(
                [
                    "solve",
                    "--scenario",
                    str(scenario_path),
                    "--max-states",
                    "100",
                    "--output",
                    str(output_path),
                ]
            )

            self.assertEqual(exit_code, 0)
            result = json.loads(output_path.read_text(encoding="utf-8"))
            self.assertTrue(result["won"])
            self.assertTrue(result["complete"])
            self.assertEqual(result["hp_loss"], 0)
            self.assertEqual(result["action_ids"], [f"play:{strike['instance_id']}:enemy_0"])

    def test_native_validation_rejects_unknown_content(self) -> None:
        scenario = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text())
        scenario["initial_state"]["hand"][0]["model_id"] = "CARD.UNKNOWN"

        with tempfile.TemporaryDirectory() as directory:
            scenario_path = Path(directory) / "scenario.json"
            scenario_path.write_text(json.dumps(scenario), encoding="utf-8")
            with self.assertRaises(SystemExit):
                main(["solve", "--scenario", str(scenario_path)])

    def test_native_validation_rejects_unknown_snapshot_fields(self) -> None:
        scenario = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text())
        scenario["initial_state"]["hidden_future_mechanic"] = {"counter": 1}

        with self.assertRaisesRegex(Exception, "unknown field"):
            solve_simulator(
                scenario,
                SolveLimits(max_states=100, max_turns=5, timeout_seconds=5),
            )

    def test_native_validation_rejects_an_unverified_rng_fingerprint(self) -> None:
        scenario = json.loads((ROOT / "examples" / "sim_nibbit.json").read_text())
        scenario["initial_state"]["rng"]["algorithm"] = "unverified_rng_deadbeef1234"

        with self.assertRaisesRegex(Exception, "unsupported mechanic: rng algorithm"):
            solve_simulator(
                scenario,
                SolveLimits(max_states=100, max_turns=5, timeout_seconds=5),
            )


if __name__ == "__main__":
    unittest.main()
