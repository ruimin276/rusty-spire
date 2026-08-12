#!/usr/bin/env python3
"""Enforce the permitted active Rust workspace dependency graph."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ALLOWED = {
    "rusty-spire-core": set(),
    "rusty-spire-data": {"rusty-spire-core"},
    "rusty-spire-combat": {"rusty-spire-core", "rusty-spire-data"},
    "rusty-spire-simulator": {"rusty-spire-core", "rusty-spire-data", "rusty-spire-combat"},
    "rusty-spire-heuristics": {"rusty-spire-core", "rusty-spire-simulator"},
    "rusty-spire-api": {
        "rusty-spire-core", "rusty-spire-data", "rusty-spire-combat",
        "rusty-spire-simulator", "rusty-spire-heuristics",
    },
    "rusty-spire-cli": {"rusty-spire-api"},
    "rusty-spire-wasm": {"rusty-spire-api"},
}


def main() -> None:
    output = subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT
    )
    packages = json.loads(output)["packages"]
    names = {package["name"] for package in packages}
    missing = ALLOWED.keys() - names
    if missing:
        raise ValueError(f"workspace is missing required packages: {', '.join(sorted(missing))}")
    for package in packages:
        name = package["name"]
        if name not in ALLOWED:
            raise ValueError(f"unexpected active package {name}")
        local = {
            dependency["name"]
            for dependency in package["dependencies"]
            if dependency.get("path") is not None
        }
        forbidden = local - ALLOWED[name]
        if forbidden:
            raise ValueError(f"{name} has forbidden dependencies: {', '.join(sorted(forbidden))}")
    print("validated active Rust workspace dependency boundaries")


if __name__ == "__main__":
    main()
