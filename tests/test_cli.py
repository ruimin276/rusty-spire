from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from sls2_combat_solver.cli import main


ROOT = Path(__file__).resolve().parents[1]


class CliTests(unittest.TestCase):
    def test_solve_writes_stable_json(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "result.json"

            exit_code = main(
                [
                    "solve",
                    "--scenario",
                    str(ROOT / "examples" / "mock_candidate.json"),
                    "--output",
                    str(output),
                ]
            )

            self.assertEqual(exit_code, 0)
            data = json.loads(output.read_text(encoding="utf-8"))
            self.assertTrue(data["won"])
            self.assertEqual(data["hp_loss"], 0)
            self.assertEqual(data["action_ids"], ["candidate_card"])

    def test_compare_writes_delta(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "comparison.json"

            exit_code = main(
                [
                    "compare",
                    "--baseline",
                    str(ROOT / "examples" / "mock_baseline.json"),
                    "--candidate",
                    str(ROOT / "examples" / "mock_candidate.json"),
                    "--output",
                    str(output),
                ]
            )

            self.assertEqual(exit_code, 0)
            data = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(data["hp_loss_delta"], -2)
            self.assertEqual(data["better"], "candidate")


if __name__ == "__main__":
    unittest.main()
