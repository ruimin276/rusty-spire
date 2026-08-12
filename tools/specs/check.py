#!/usr/bin/env python3
"""Validate normative specification metadata and traceability."""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPECS = ROOT / "specs"
METADATA = re.compile(
    r"\A---\n(?P<body>.*?)\n---\n", re.DOTALL
)
REQUIREMENT = re.compile(r"^### ([A-Z][A-Z0-9]*-[0-9]{3})\b", re.MULTILINE)


def parse_metadata(path: Path) -> dict[str, str]:
    text = path.read_text(encoding="utf-8")
    match = METADATA.match(text)
    if not match:
        raise ValueError(f"{path.relative_to(ROOT)} has no metadata header")
    result: dict[str, str] = {}
    for line in match.group("body").splitlines():
        key, separator, value = line.partition(":")
        if not separator:
            raise ValueError(f"{path.relative_to(ROOT)} has invalid metadata: {line}")
        result[key.strip()] = value.strip()
    for key in ("id", "title", "status", "depends"):
        if key not in result:
            raise ValueError(f"{path.relative_to(ROOT)} is missing {key}")
    if result["status"] not in {"accepted", "draft", "retired"}:
        raise ValueError(f"{path.relative_to(ROOT)} has invalid status")
    return result


def main() -> None:
    manifest = json.loads((SPECS / "traceability.json").read_text(encoding="utf-8"))
    if manifest.get("schema_version") != 1:
        raise ValueError("traceability schema_version must be 1")
    mappings = manifest.get("requirements")
    if not isinstance(mappings, dict):
        raise ValueError("traceability requirements must be an object")
    registry = json.loads((SPECS / "checks.json").read_text(encoding="utf-8"))
    if registry.get("schema_version") != 1 or not isinstance(registry.get("checks"), dict):
        raise ValueError("checks registry must have schema_version 1 and a checks object")
    registered_checks = registry["checks"]
    for check_id, definition in registered_checks.items():
        if not re.fullmatch(r"(?:test|check):[a-z0-9_]+", check_id):
            raise ValueError(f"invalid registered check id {check_id}")
        if not isinstance(definition, dict) or not isinstance(definition.get("command"), str):
            raise ValueError(f"{check_id} must define a command")
        sources = definition.get("sources")
        if not isinstance(sources, list) or not sources:
            raise ValueError(f"{check_id} must reference test/check sources")
        missing_sources = [source for source in sources if not (ROOT / source).is_file()]
        if missing_sources:
            raise ValueError(f"{check_id} references missing sources: {', '.join(missing_sources)}")
    schema_ids: set[str] = set()
    for path in sorted((SPECS / "schemas").glob("*.schema.json")):
        schema = json.loads(path.read_text(encoding="utf-8"))
        schema_id = schema.get("$id")
        if not isinstance(schema_id, str) or schema_id in schema_ids:
            raise ValueError(f"{path.relative_to(ROOT)} has a missing or duplicate $id")
        schema_ids.add(schema_id)
    if len(schema_ids) < 5:
        raise ValueError("expected the five versioned public schemas")

    spec_ids: set[str] = set()
    accepted: set[str] = set()
    all_requirements: set[str] = set()
    dependencies: dict[str, list[str]] = {}
    for path in sorted(SPECS.glob("[0-9][0-9][0-9]-*.md")):
        metadata = parse_metadata(path)
        spec_id = metadata["id"]
        if spec_id in spec_ids:
            raise ValueError(f"duplicate spec id {spec_id}")
        spec_ids.add(spec_id)
        raw_dependencies = metadata["depends"].removeprefix("[").removesuffix("]")
        dependencies[spec_id] = [value.strip() for value in raw_dependencies.split(",") if value.strip()]
        requirements = set(REQUIREMENT.findall(path.read_text(encoding="utf-8")))
        duplicate = all_requirements & requirements
        if duplicate:
            raise ValueError(f"duplicate requirement ids: {', '.join(sorted(duplicate))}")
        all_requirements |= requirements
        if metadata["status"] == "accepted":
            if not requirements:
                raise ValueError(f"accepted {spec_id} has no requirements")
            accepted |= requirements

    for spec_id, values in dependencies.items():
        missing = set(values) - spec_ids
        if missing:
            raise ValueError(f"{spec_id} depends on unknown specs: {', '.join(sorted(missing))}")
    missing_mappings = accepted - mappings.keys()
    extra_mappings = mappings.keys() - accepted
    if missing_mappings:
        raise ValueError(f"unmapped accepted requirements: {', '.join(sorted(missing_mappings))}")
    if extra_mappings:
        raise ValueError(f"traceability maps non-accepted requirements: {', '.join(sorted(extra_mappings))}")
    for requirement, checks in mappings.items():
        if not isinstance(checks, list) or not checks or not all(
            isinstance(check, str) and re.fullmatch(r"(?:test|check):[a-z0-9_]+", check)
            for check in checks
        ):
            raise ValueError(f"{requirement} has invalid checks")
        missing_checks = set(checks) - registered_checks.keys()
        if missing_checks:
            raise ValueError(
                f"{requirement} references unregistered checks: {', '.join(sorted(missing_checks))}"
            )
    print(
        f"validated {len(spec_ids)} specs, {len(accepted)} accepted requirements, "
        f"and {len(registered_checks)} registered checks"
    )


if __name__ == "__main__":
    main()
