#!/usr/bin/env python3
"""Validate normative specification format, links, and traceability."""

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
REQUIREMENT_HEADING = re.compile(
    r"^### ([A-Z][A-Z0-9]*-[0-9]{3})(?P<suffix>.*)$", re.MULTILINE
)
CHECK_REFERENCE = re.compile(r"`((?:test|check):[a-z0-9_]+)`")
NORMATIVE_KEYWORD = re.compile(r"\b(?:MUST|SHOULD|MAY)\b")
MARKDOWN_LINK = re.compile(r"(?<!!)\[[^]]+\]\(([^)]+)\)")
ACTIVE_STATUSES = {"accepted", "draft"}
KNOWN_STATUSES = ACTIVE_STATUSES | {"retired"}
REQUIRED_SECTIONS = ("Status", "Summary", "Specification", "Conformance", "References")
MAX_ACTIVE_SPECS = 15
MAX_ACTIVE_LINES = 350
MAX_ACTIVE_WORDS = 2_500


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
        key = key.strip()
        if not key or key in result:
            raise ValueError(
                f"{path.relative_to(ROOT)} has empty or duplicate metadata key {key!r}"
            )
        result[key] = value.strip()
    for key in (
        "id",
        "title",
        "status",
        "domain",
        "version",
        "applies_to",
        "depends",
        "sources",
    ):
        if key not in result:
            raise ValueError(f"{path.relative_to(ROOT)} is missing {key}")
    if not re.fullmatch(r"SPEC-[0-9]{3}", result["id"]):
        raise ValueError(f"{path.relative_to(ROOT)} has invalid spec id {result['id']}")
    if result["status"] not in KNOWN_STATUSES:
        raise ValueError(f"{path.relative_to(ROOT)} has invalid status")
    if not result["version"].isdigit() or int(result["version"]) < 1:
        raise ValueError(f"{path.relative_to(ROOT)} has invalid version")
    if not re.fullmatch(r"[a-z][a-z0-9-]*", result["domain"]):
        raise ValueError(f"{path.relative_to(ROOT)} has invalid domain")
    if not result["applies_to"]:
        raise ValueError(f"{path.relative_to(ROOT)} has empty domain or applies_to")
    return result


def parse_list(value: str, field: str, path: Path) -> list[str]:
    if not value.startswith("[") or not value.endswith("]"):
        raise ValueError(f"{path.relative_to(ROOT)} {field} must use bracket syntax")
    return [item.strip() for item in value[1:-1].split(",") if item.strip()]


def section_positions(path: Path, text: str) -> dict[str, int]:
    positions: dict[str, int] = {}
    for section in REQUIRED_SECTIONS:
        matches = list(re.finditer(rf"^## {re.escape(section)}\s*$", text, re.MULTILINE))
        if len(matches) != 1:
            raise ValueError(
                f"{path.relative_to(ROOT)} must contain exactly one ## {section} section"
            )
        positions[section] = matches[0].start()
    if list(positions.values()) != sorted(positions.values()):
        raise ValueError(
            f"{path.relative_to(ROOT)} sections must follow: {', '.join(REQUIRED_SECTIONS)}"
        )
    return positions


def validate_spec_format(
    path: Path, metadata: dict[str, str]
) -> tuple[str, set[str], dict[str, set[str]]]:
    text = path.read_text(encoding="utf-8")
    positions = section_positions(path, text)
    heading = re.search(r"^# (SPEC-[0-9]{3}): (.+)$", text, re.MULTILINE)
    if not heading or heading.group(1) != metadata["id"] or heading.group(2) != metadata["title"]:
        raise ValueError(
            f"{path.relative_to(ROOT)} title must be '# {metadata['id']}: {metadata['title']}'"
        )
    status_body = text[positions["Status"] : positions["Summary"]]
    stated_status = next(
        (line.strip() for line in status_body.splitlines()[1:] if line.strip()), ""
    )
    if stated_status != metadata["status"].upper():
        raise ValueError(
            f"{path.relative_to(ROOT)} status section does not match metadata"
        )
    summary_match = re.search(
        r"^## Summary\s*$\n(?P<body>.*?)(?=^## |\Z)", text, re.MULTILINE | re.DOTALL
    )
    summary_words = re.findall(r"\b[\w'-]+\b", summary_match.group("body")) if summary_match else []
    if not summary_words or len(summary_words) > 100:
        raise ValueError(f"{path.relative_to(ROOT)} summary must contain 1-100 words")
    if metadata["status"] in ACTIVE_STATUSES:
        lines = len(text.splitlines())
        words = len(re.findall(r"\b[\w'-]+\b", text))
        if lines > MAX_ACTIVE_LINES or words > MAX_ACTIVE_WORDS:
            raise ValueError(
                f"{path.relative_to(ROOT)} exceeds the active spec budget "
                f"({lines}/{MAX_ACTIVE_LINES} lines, {words}/{MAX_ACTIVE_WORDS} words)"
            )
    requirement_list = REQUIREMENT.findall(text)
    duplicate_requirements = sorted(
        {requirement for requirement in requirement_list if requirement_list.count(requirement) > 1}
    )
    if duplicate_requirements:
        raise ValueError(
            f"{path.relative_to(ROOT)} repeats requirement ids: "
            f"{', '.join(duplicate_requirements)}"
        )
    if requirement_list != sorted(requirement_list):
        raise ValueError(f"{path.relative_to(ROOT)} requirements are not in ID order")
    requirements = set(requirement_list)
    if len(requirements) > 20:
        raise ValueError(
            f"{path.relative_to(ROOT)} exceeds the 20-requirement active-spec budget"
        )
    malformed_headings = [
        requirement
        for requirement, suffix in REQUIREMENT_HEADING.findall(text)
        if not re.fullmatch(r" — \S.*", suffix)
    ]
    if malformed_headings:
        raise ValueError(
            f"{path.relative_to(ROOT)} has malformed requirement headings: "
            f"{', '.join(sorted(malformed_headings))}"
        )
    specification = text[positions["Specification"] : positions["Conformance"]]
    outside = REQUIREMENT.findall(text[: positions["Specification"]] + text[positions["Conformance"] :])
    if outside:
        raise ValueError(
            f"{path.relative_to(ROOT)} has requirements outside Specification: "
            f"{', '.join(outside)}"
        )
    if requirements and not REQUIREMENT.search(specification):
        raise ValueError(f"{path.relative_to(ROOT)} has no requirements in Specification")
    if metadata["status"] == "accepted":
        h3_matches = list(re.finditer(r"^### (.+)$", specification, re.MULTILINE))
        missing_keywords: list[str] = []
        unnumbered_keywords: list[str] = []
        preamble_end = h3_matches[0].start() if h3_matches else len(specification)
        if NORMATIVE_KEYWORD.search(specification[:preamble_end]):
            unnumbered_keywords.append("Specification preamble")
        for index, match in enumerate(h3_matches):
            end = (
                h3_matches[index + 1].start()
                if index + 1 < len(h3_matches)
                else len(specification)
            )
            body = specification[match.end() : end]
            requirement = re.match(r"([A-Z][A-Z0-9]*-[0-9]{3})\b", match.group(1))
            if requirement and not NORMATIVE_KEYWORD.search(body):
                missing_keywords.append(requirement.group(1))
            if not requirement and NORMATIVE_KEYWORD.search(body):
                unnumbered_keywords.append(match.group(1))
        if missing_keywords:
            raise ValueError(
                f"{path.relative_to(ROOT)} accepted requirements lack normative keywords: "
                f"{', '.join(missing_keywords)}"
            )
        if unnumbered_keywords:
            raise ValueError(
                f"{path.relative_to(ROOT)} has normative keywords outside numbered "
                f"requirements: {', '.join(unnumbered_keywords)}"
            )
    conformance = text[positions["Conformance"] : positions["References"]]
    conformance_rows: dict[str, list[str]] = {}
    for line in conformance.splitlines():
        row = re.match(r"^\|\s*([A-Z][A-Z0-9]*-[0-9]{3})\s*\|", line)
        if row:
            conformance_rows.setdefault(row.group(1), []).append(line)
    unknown_conformance = conformance_rows.keys() - requirements
    if unknown_conformance:
        raise ValueError(
            f"{path.relative_to(ROOT)} conformance has unknown requirements: "
            f"{', '.join(sorted(unknown_conformance))}"
        )
    duplicate_conformance = [
        requirement for requirement, rows in conformance_rows.items() if len(rows) != 1
    ]
    if duplicate_conformance:
        raise ValueError(
            f"{path.relative_to(ROOT)} conformance repeats: "
            f"{', '.join(sorted(duplicate_conformance))}"
        )
    missing_conformance = requirements - conformance_rows.keys()
    if missing_conformance:
        raise ValueError(
            f"{path.relative_to(ROOT)} conformance omits: {', '.join(sorted(missing_conformance))}"
        )
    conformance_checks = {
        requirement: set(CHECK_REFERENCE.findall(rows[0]))
        for requirement, rows in conformance_rows.items()
    }
    return text, requirements, conformance_checks


def requirement_summary(requirements: set[str]) -> str:
    ordered = sorted(requirements)
    if not ordered:
        return "—"
    prefixes = {value.rsplit("-", 1)[0] for value in ordered}
    numbers = [int(value.rsplit("-", 1)[1]) for value in ordered]
    if (
        len(prefixes) == 1
        and len(ordered) > 1
        and numbers == list(range(numbers[0], numbers[-1] + 1))
    ):
        return f"{ordered[0]}–{ordered[-1]}"
    return ", ".join(ordered)


def validate_index(
    path: Path,
    text: str,
    metadata_by_id: dict[str, dict[str, str]],
    paths: dict[str, Path],
    requirements_by_id: dict[str, set[str]],
    dependencies: dict[str, list[str]],
) -> None:
    headings = {
        "accepted": text.find("## Accepted specifications"),
        "draft": text.find("## Draft specifications"),
        "retired": text.find("## Retired specifications"),
    }
    for status in {metadata["status"] for metadata in metadata_by_id.values()}:
        if headings[status] < 0:
            raise ValueError(f"spec index is missing the {status} specifications section")

    rows: dict[str, tuple[list[str], int]] = {}
    for line in text.splitlines():
        if not line.startswith("| [SPEC-"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 7:
            raise ValueError(f"spec index row must have seven columns: {line}")
        link = re.fullmatch(r"\[(SPEC-[0-9]{3})\]\(([^)]+)\)", cells[0])
        if not link:
            raise ValueError(f"spec index has malformed spec link: {cells[0]}")
        spec_id = link.group(1)
        if spec_id in rows:
            raise ValueError(f"spec index contains duplicate row {spec_id}")
        rows[spec_id] = (cells, text.find(line))

    missing = metadata_by_id.keys() - rows.keys()
    extra = rows.keys() - metadata_by_id.keys()
    if missing:
        raise ValueError(f"spec index omits: {', '.join(sorted(missing))}")
    if extra:
        raise ValueError(f"spec index lists unknown specs: {', '.join(sorted(extra))}")

    for spec_id, metadata in metadata_by_id.items():
        cells, position = rows[spec_id]
        expected = [
            f"[{spec_id}]({paths[spec_id].name})",
            metadata["title"],
            metadata["domain"],
            metadata["status"].upper(),
            requirement_summary(requirements_by_id[spec_id]),
            ", ".join(dependencies[spec_id]) or "—",
            str(len(paths[spec_id].read_text(encoding="utf-8").splitlines())),
        ]
        if cells != expected:
            raise ValueError(
                f"spec index row for {spec_id} is stale\n"
                f"expected: | {' | '.join(expected)} |"
            )
        section_start = headings[metadata["status"]]
        later_sections = [value for value in headings.values() if value > section_start]
        section_end = min(later_sections, default=len(text))
        if not section_start < position < section_end:
            raise ValueError(
                f"spec index lists {spec_id} outside its {metadata['status']} section"
            )


def validate_relative_links(path: Path, text: str) -> None:
    for target in MARKDOWN_LINK.findall(text):
        target = target.strip()
        if target.startswith("<") and target.endswith(">"):
            target = target[1:-1]
        else:
            target = target.split(maxsplit=1)[0]
        if target.startswith(("http://", "https://", "mailto:", "#")):
            continue
        relative, separator, anchor = target.partition("#")
        resolved = (path.parent / relative).resolve() if relative else path.resolve()
        if not resolved.is_relative_to(ROOT):
            raise ValueError(
                f"{path.relative_to(ROOT)} link escapes the repository: {relative}"
            )
        if relative and not resolved.exists():
            raise ValueError(
                f"{path.relative_to(ROOT)} links to missing path {relative}"
            )
        if separator and anchor and resolved.is_file() and resolved.suffix == ".md":
            target_text = resolved.read_text(encoding="utf-8")
            anchors: set[str] = set()
            for heading in re.findall(r"^#{1,6}\s+(.+?)\s*$", target_text, re.MULTILINE):
                plain = re.sub(r"[`*_~]", "", heading).lower().strip()
                anchors.add(re.sub(r"[^a-z0-9 _-]", "", plain).replace(" ", "-"))
            if anchor not in anchors:
                raise ValueError(
                    f"{path.relative_to(ROOT)} links to missing anchor #{anchor} in "
                    f"{resolved.relative_to(ROOT)}"
                )


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
        if (
            not isinstance(definition, dict)
            or not isinstance(definition.get("command"), str)
            or not definition["command"].strip()
        ):
            raise ValueError(f"{check_id} must define a command")
        sources = definition.get("sources")
        if not isinstance(sources, list) or not sources:
            raise ValueError(f"{check_id} must reference test/check sources")
        invalid_sources = [
            source
            for source in sources
            if not isinstance(source, str)
            or not (ROOT / source).resolve().is_relative_to(ROOT)
        ]
        if invalid_sources:
            raise ValueError(
                f"{check_id} has invalid repository-relative sources: "
                f"{', '.join(map(str, invalid_sources))}"
            )
        missing_sources = [source for source in sources if not (ROOT / source).is_file()]
        if missing_sources:
            raise ValueError(f"{check_id} references missing sources: {', '.join(missing_sources)}")
        symbols = definition.get("symbols", [])
        if not isinstance(symbols, list) or not all(
            isinstance(symbol, str) and symbol for symbol in symbols
        ):
            raise ValueError(f"{check_id} has invalid symbols")
        if check_id.startswith("test:") and not symbols:
            raise ValueError(f"{check_id} must name one or more exact test symbols")
        source_text = "\n".join(
            (ROOT / source).read_text(encoding="utf-8", errors="replace") for source in sources
        )
        missing_symbols = [symbol for symbol in symbols if symbol not in source_text]
        if missing_symbols:
            raise ValueError(
                f"{check_id} references missing symbols: {', '.join(missing_symbols)}"
            )
    schema_ids: set[str] = set()
    for path in sorted((SPECS / "schemas").glob("*.schema.json")):
        schema = json.loads(path.read_text(encoding="utf-8"))
        schema_id = schema.get("$id")
        if not isinstance(schema_id, str) or schema_id in schema_ids:
            raise ValueError(f"{path.relative_to(ROOT)} has a missing or duplicate $id")
        schema_ids.add(schema_id)
    if len(schema_ids) < 6:
        raise ValueError("expected the six versioned public schemas")

    spec_ids: set[str] = set()
    spec_paths: dict[str, Path] = {}
    spec_metadata: dict[str, dict[str, str]] = {}
    spec_requirements: dict[str, set[str]] = {}
    conformance_checks: dict[str, set[str]] = {}
    accepted: set[str] = set()
    all_requirements: set[str] = set()
    dependencies: dict[str, list[str]] = {}
    statuses: dict[str, str] = {}
    active_count = 0
    root_markdown = sorted(path for path in SPECS.glob("*.md") if path.name != "README.md")
    malformed_spec_files = [
        path.name
        for path in root_markdown
        if not re.fullmatch(r"[0-9]{3}-[a-z0-9]+(?:-[a-z0-9]+)*\.md", path.name)
    ]
    if malformed_spec_files:
        raise ValueError(
            f"malformed root specification filenames: {', '.join(malformed_spec_files)}"
        )
    for path in root_markdown:
        metadata = parse_metadata(path)
        spec_id = metadata["id"]
        if spec_id in spec_ids:
            raise ValueError(f"duplicate spec id {spec_id}")
        spec_ids.add(spec_id)
        spec_paths[spec_id] = path
        spec_metadata[spec_id] = metadata
        statuses[spec_id] = metadata["status"]
        expected_prefix = spec_id.removeprefix("SPEC-") + "-"
        if not path.name.startswith(expected_prefix):
            raise ValueError(
                f"{path.relative_to(ROOT)} filename prefix does not match {spec_id}"
            )
        if metadata["status"] in ACTIVE_STATUSES:
            active_count += 1
        dependencies[spec_id] = parse_list(metadata["depends"], "depends", path)
        sources = parse_list(metadata["sources"], "sources", path)
        if not sources:
            raise ValueError(f"{path.relative_to(ROOT)} must name at least one source")
        invalid_sources = [
            source
            for source in sources
            if not (ROOT / source).resolve().is_relative_to(ROOT)
        ]
        if invalid_sources:
            raise ValueError(
                f"{path.relative_to(ROOT)} metadata sources escape repository: "
                f"{', '.join(invalid_sources)}"
            )
        missing_sources = [source for source in sources if not (ROOT / source).exists()]
        if missing_sources:
            raise ValueError(
                f"{path.relative_to(ROOT)} metadata references missing sources: "
                f"{', '.join(missing_sources)}"
            )
        text, requirements, mentioned_checks = validate_spec_format(path, metadata)
        validate_relative_links(path, text)
        unknown_mentions = {
            check
            for checks in mentioned_checks.values()
            for check in checks
            if check not in registered_checks
        }
        if unknown_mentions:
            raise ValueError(
                f"{path.relative_to(ROOT)} conformance references unregistered checks: "
                f"{', '.join(sorted(unknown_mentions))}"
            )
        spec_requirements[spec_id] = requirements
        conformance_checks.update(mentioned_checks)
        if metadata["status"] != "accepted" and any(mentioned_checks.values()):
            raise ValueError(
                f"{path.relative_to(ROOT)} non-accepted conformance references checks"
            )
        duplicate = all_requirements & requirements
        if duplicate:
            raise ValueError(f"duplicate requirement ids: {', '.join(sorted(duplicate))}")
        all_requirements |= requirements
        if metadata["status"] == "accepted":
            if not requirements:
                raise ValueError(f"accepted {spec_id} has no requirements")
            accepted |= requirements

    if active_count > MAX_ACTIVE_SPECS:
        raise ValueError(
            f"active specification count {active_count} exceeds {MAX_ACTIVE_SPECS}"
        )
    index_text = (SPECS / "README.md").read_text(encoding="utf-8")
    validate_relative_links(SPECS / "README.md", index_text)
    validate_index(
        SPECS / "README.md",
        index_text,
        spec_metadata,
        spec_paths,
        spec_requirements,
        dependencies,
    )
    agents = ROOT / "AGENTS.md"
    if not agents.is_file():
        raise ValueError("AGENTS.md routing index is required")
    agents_text = agents.read_text(encoding="utf-8")
    validate_relative_links(agents, agents_text)
    if "specs/README.md" not in agents_text or "000-specification-guidelines.md" not in agents_text:
        raise ValueError("AGENTS.md must route agents to the spec index and writing guidelines")
    for spec_id, path in spec_paths.items():
        if f"[{spec_id}](specs/{path.name})" not in agents_text:
            raise ValueError(f"AGENTS.md does not route to {spec_id}")

    for spec_id, values in dependencies.items():
        missing = set(values) - spec_ids
        if missing:
            raise ValueError(f"{spec_id} depends on unknown specs: {', '.join(sorted(missing))}")
        if statuses[spec_id] == "accepted":
            nonbinding = [value for value in values if statuses[value] != "accepted"]
            if nonbinding:
                raise ValueError(
                    f"accepted {spec_id} depends on non-accepted specs: "
                    f"{', '.join(sorted(nonbinding))}"
                )

    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(spec_id: str) -> None:
        if spec_id in visiting:
            raise ValueError(f"specification dependency cycle includes {spec_id}")
        if spec_id in visited:
            return
        visiting.add(spec_id)
        for dependency in dependencies[spec_id]:
            visit(dependency)
        visiting.remove(spec_id)
        visited.add(spec_id)

    for spec_id in spec_ids:
        visit(spec_id)
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
        documented_checks = conformance_checks.get(requirement, set())
        if documented_checks and documented_checks != set(checks):
            raise ValueError(
                f"{requirement} conformance and traceability checks differ: "
                f"{', '.join(sorted(documented_checks))} != {', '.join(sorted(checks))}"
            )
    used_checks = {check for checks in mappings.values() for check in checks}
    unused_checks = registered_checks.keys() - used_checks
    if unused_checks:
        raise ValueError(
            f"registered checks have no requirement mapping: "
            f"{', '.join(sorted(unused_checks))}"
        )
    print(
        f"validated {len(spec_ids)} specs, {len(accepted)} accepted requirements, "
        f"and {len(registered_checks)} registered checks"
    )


if __name__ == "__main__":
    main()
