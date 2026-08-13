from __future__ import annotations

import importlib.util
import json
import re
import sys
import tempfile
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
content_status = load_module(
    "generate_content_status", ROOT / "tools/specs/generate_content_status.py"
)
spec_checker = load_module("spec_checker", ROOT / "tools/specs/check.py")


class GovernanceTests(unittest.TestCase):
    def test_ci_has_no_performance_gate(self) -> None:
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        jobs = workflow.split("\njobs:\n", 1)[1]
        job_ids = re.findall(r"(?m)^  ([a-zA-Z0-9_-]+):\s*$", jobs)
        self.assertFalse(
            any(re.search(r"(?:^|[-_])(?:perf|bench|benchmark)(?:$|[-_])", job) for job in job_ids)
        )
        prohibited = ("cargo bench", "criterion", "hyperfine", "iai-callgrind", "benchmark")
        normalized = " ".join(workflow.lower().split())
        for marker in prohibited:
            self.assertNotIn(marker, normalized)

    def test_spec_checker_rejects_duplicate_and_unnumbered_normative_rules(self) -> None:
        def document(specification: str) -> str:
            return f"""---
id: SPEC-999
title: Checker Fixture
status: accepted
domain: test
version: 1
applies_to: test
depends: []
sources: [Cargo.toml]
---

# SPEC-999: Checker Fixture

## Status

ACCEPTED

## Summary

Exercises structural validation.

## Specification

{specification}

## Conformance

| Requirement | Evidence |
|---|---|
| TST-001 | fixture |

## References

- `Cargo.toml`
"""

        metadata = {"id": "SPEC-999", "title": "Checker Fixture", "status": "accepted"}
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            path = Path(directory) / "999-checker-fixture.md"
            path.write_text(
                document(
                    "### TST-001 — First rule\n\nThe fixture MUST pass.\n\n"
                    "### TST-001 — Reused rule\n\nThe fixture MUST fail."
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "repeats requirement ids"):
                spec_checker.validate_spec_format(path, metadata)

            path.write_text(
                document(
                    "### Unnumbered policy\n\nThis MUST fail.\n\n"
                    "### TST-001 — Numbered rule\n\nThe fixture MUST pass."
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(ValueError, "outside numbered requirements"):
                spec_checker.validate_spec_format(path, metadata)

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

    def test_content_status_ledgers_cover_implemented_and_inert_publication(self) -> None:
        self.assertEqual(
            content_status.PUBLISHED.read_text(encoding="utf-8"),
            content_status.encoded(),
        )
        published = content_status.published_document()
        self.assertEqual(len(published["cards"]), 12)
        self.assertEqual(len(published["relics"]), 3)
        self.assertEqual(
            published["cards"]["CARD.ADRENALINE"]["implementation_status"],
            "implemented",
        )
        self.assertEqual(
            published["relics"]["RELIC.RING_OF_THE_SNAKE"]["implementation_status"],
            "implemented",
        )
        self.assertEqual(
            published["relics"]["RELIC.BURNING_BLOOD"]["implementation_status"],
            "recognized_inert",
        )


if __name__ == "__main__":
    unittest.main()
