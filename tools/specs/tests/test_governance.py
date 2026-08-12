from __future__ import annotations

import importlib.util
import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


promote = load_module("promote", ROOT / "tools/spire-codex/promote.py")
contracts = load_module("generate_contracts", ROOT / "tools/specs/generate_contracts.py")


class GovernanceTests(unittest.TestCase):
    def test_package_promotion_is_deterministic_and_contains_proof_slice(self) -> None:
        first = promote.encoded(
            promote.promote(promote.DEFAULT_EVIDENCE, promote.DEFAULT_REVIEW, promote.DEFAULT_BASE)
        )
        second = promote.encoded(
            promote.promote(promote.DEFAULT_EVIDENCE, promote.DEFAULT_REVIEW, promote.DEFAULT_BASE)
        )
        self.assertEqual(first, second)
        package = json.loads(first)
        self.assertEqual(
            package["cards"]["CARD.IRON_WAVE"]["effects"][0]["type"], "block"
        )
        self.assertEqual(
            package["cards"]["CARD.ADRENALINE"]["effects"][-1]["type"], "draw"
        )

    def test_generated_contract_outputs_are_current(self) -> None:
        for path, expected in contracts.outputs().items():
            self.assertEqual(path.read_text(encoding="utf-8"), expected)


if __name__ == "__main__":
    unittest.main()
