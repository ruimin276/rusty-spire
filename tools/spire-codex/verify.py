#!/usr/bin/env python3
"""Offline verification for the committed data package."""

from __future__ import annotations

import sys
from pathlib import Path

from promote import DEFAULT_BASE, DEFAULT_EVIDENCE, DEFAULT_OUTPUT, DEFAULT_REVIEW, encoded, promote


def main() -> None:
    expected = encoded(promote(DEFAULT_EVIDENCE, DEFAULT_REVIEW, DEFAULT_BASE))
    actual = DEFAULT_OUTPUT.read_bytes()
    if actual != expected:
        print(f"{DEFAULT_OUTPUT.relative_to(Path.cwd())} is stale; run tools/spire-codex/promote.py", file=sys.stderr)
        raise SystemExit(1)
    print("verified deterministic data package")


if __name__ == "__main__":
    main()
