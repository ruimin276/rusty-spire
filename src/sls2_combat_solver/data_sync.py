from __future__ import annotations

import hashlib
import json
import re
import shutil
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from uuid import uuid4


SOURCE_NAME = "spire_codex"
NORMALIZER_VERSION = 1
DEFAULT_BASE_URL = "https://spire-codex.com"
DEFAULT_USER_AGENT = "sls2-card-benchmark-data-sync/0.1"
ENTITY_ENDPOINTS = ("cards", "relics", "monsters")
MODEL_PREFIXES = {"cards": "CARD", "relics": "RELIC", "monsters": "MONSTER"}
REQUIRED_FIELDS = {
    "cards": frozenset(("id", "name", "cost", "type", "rarity")),
    "relics": frozenset(("id", "name", "description", "rarity")),
    "monsters": frozenset(("id", "name", "type", "moves")),
}
MAX_RESPONSE_BYTES = 20 * 1024 * 1024


class DataSyncError(RuntimeError):
    """Raised when an upstream response cannot produce a trustworthy snapshot."""


@dataclass(frozen=True)
class FetchResult:
    endpoint: str
    url: str
    body: bytes
    payload: Any
    headers: dict[str, str]


@dataclass(frozen=True)
class SnapshotResult:
    path: Path
    manifest: dict[str, Any]
    created: bool


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _canonical_json(data: Any) -> bytes:
    return json.dumps(
        data,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def _parse_retry_after(value: str | None, fallback: float) -> float:
    if value is None:
        return fallback
    try:
        return max(float(value), 0.0)
    except ValueError:
        return fallback


def _fetch_json(
    base_url: str,
    endpoint: str,
    api_path: str,
    *,
    user_agent: str,
    timeout_seconds: float,
    attempts: int,
) -> FetchResult:
    url = f"{base_url.rstrip('/')}/api/{api_path}"
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/json",
            "User-Agent": user_agent,
        },
    )
    for attempt in range(attempts):
        try:
            with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
                declared_length = response.headers.get("Content-Length")
                if declared_length and int(declared_length) > MAX_RESPONSE_BYTES:
                    raise DataSyncError(
                        f"{endpoint} response declares {declared_length} bytes; "
                        f"limit is {MAX_RESPONSE_BYTES}"
                    )
                body = response.read(MAX_RESPONSE_BYTES + 1)
                if len(body) > MAX_RESPONSE_BYTES:
                    raise DataSyncError(
                        f"{endpoint} response exceeds {MAX_RESPONSE_BYTES} bytes"
                    )
                try:
                    payload = json.loads(body)
                except (UnicodeDecodeError, json.JSONDecodeError) as exc:
                    raise DataSyncError(f"{endpoint} returned invalid JSON: {exc}") from exc
                selected_headers = {
                    key.lower(): value
                    for key, value in response.headers.items()
                    if key.lower()
                    in {
                        "content-type",
                        "etag",
                        "last-modified",
                        "x-ratelimit-limit",
                        "x-ratelimit-remaining",
                        "x-ratelimit-reset",
                    }
                }
                return FetchResult(
                    endpoint, response.geturl(), body, payload, selected_headers
                )
        except urllib.error.HTTPError as exc:
            retryable = exc.code == 429 or 500 <= exc.code < 600
            if not retryable or attempt + 1 == attempts:
                raise DataSyncError(f"{endpoint} request failed with HTTP {exc.code}") from exc
            time.sleep(_parse_retry_after(exc.headers.get("Retry-After"), 2**attempt))
        except urllib.error.URLError as exc:
            if attempt + 1 == attempts:
                raise DataSyncError(f"{endpoint} request failed: {exc.reason}") from exc
            time.sleep(2**attempt)
    raise AssertionError("retry loop exited unexpectedly")


def _validate_collection(kind: str, payload: Any) -> list[dict[str, Any]]:
    if not isinstance(payload, list) or not payload:
        raise DataSyncError(f"{kind} must be a non-empty JSON array")
    required = REQUIRED_FIELDS[kind]
    records: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    for index, value in enumerate(payload):
        if not isinstance(value, dict):
            raise DataSyncError(f"{kind}[{index}] must be an object")
        missing = sorted(required - value.keys())
        if missing:
            raise DataSyncError(
                f"{kind}[{index}] is missing fields: {', '.join(missing)}"
            )
        source_id = value["id"]
        if not isinstance(source_id, str) or not source_id:
            raise DataSyncError(f"{kind}[{index}].id must be a non-empty string")
        if source_id in seen_ids:
            raise DataSyncError(f"{kind} contains duplicate id {source_id!r}")
        seen_ids.add(source_id)
        records.append(value)
    return records


def _normalize_collection(
    kind: str, records: list[dict[str, Any]], channel: str
) -> list[dict[str, Any]]:
    prefix = MODEL_PREFIXES[kind]
    normalized = []
    for record in sorted(records, key=lambda item: item["id"]):
        source_id = record["id"]
        normalized.append(
            {
                "model_id": f"{prefix}.{source_id}",
                "source": f"{SOURCE_NAME}:{channel}",
                "source_id": source_id,
                **{key: value for key, value in record.items() if key != "id"},
            }
        )
    return normalized


def _version_evidence(changelogs: Any) -> dict[str, Any]:
    if not isinstance(changelogs, list):
        raise DataSyncError("changelogs must be a JSON array")
    version_pattern = re.compile(r"\bv(\d+\.\d+(?:\.\d+)?)\b", re.IGNORECASE)
    for entry in changelogs:
        if not isinstance(entry, dict) or not entry.get("build_id"):
            continue
        match = version_pattern.search(str(entry.get("title", "")))
        return {
            "status": "inferred_from_latest_changelog_with_build_id",
            "game_version": f"v{match.group(1)}" if match else None,
            "steam_build_id": str(entry["build_id"]),
            "source_release": str(entry.get("tag", entry.get("game_version", ""))),
            "source_release_date": entry.get("date"),
            "changelog_title": entry.get("title"),
        }
    return {
        "status": "unavailable",
        "game_version": None,
        "steam_build_id": None,
        "source_release": None,
        "source_release_date": None,
        "changelog_title": None,
    }


def _write_json(path: Path, payload: Any) -> None:
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def _load_latest(output_root: Path, channel: str) -> dict[str, Any] | None:
    latest_path = output_root / SOURCE_NAME / channel / "latest.json"
    if not latest_path.exists():
        return None
    try:
        value = json.loads(latest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def fetch_spire_codex_snapshot(
    output_root: Path,
    *,
    base_url: str = DEFAULT_BASE_URL,
    user_agent: str = DEFAULT_USER_AGENT,
    timeout_seconds: float = 30.0,
    delay_seconds: float = 1.05,
    attempts: int = 3,
    force: bool = False,
    channel: str = "stable",
    now: datetime | None = None,
) -> SnapshotResult:
    """Fetch, validate, normalize, and immutably save a Spire Codex snapshot."""
    if timeout_seconds <= 0:
        raise ValueError("timeout_seconds must be positive")
    if delay_seconds < 0:
        raise ValueError("delay_seconds cannot be negative")
    if attempts <= 0:
        raise ValueError("attempts must be positive")
    if channel not in {"stable", "beta"}:
        raise ValueError("channel must be 'stable' or 'beta'")

    fetched: dict[str, FetchResult] = {}
    requests = (
        (
            ("changelogs", "changelogs"),
            ("cards", "cards"),
            ("relics", "relics"),
            ("monsters", "monsters"),
        )
        if channel == "stable"
        else (
            ("beta_version", "beta/version"),
            ("beta_diff", "beta/diff"),
            ("cards", "cards?channel=beta"),
            ("relics", "relics?channel=beta"),
            ("monsters", "monsters?channel=beta"),
        )
    )
    for index, (endpoint, api_path) in enumerate(requests):
        if index and delay_seconds:
            time.sleep(delay_seconds)
        fetched[endpoint] = _fetch_json(
            base_url,
            endpoint,
            api_path,
            user_agent=user_agent,
            timeout_seconds=timeout_seconds,
            attempts=attempts,
        )

    collections = {
        kind: _validate_collection(kind, fetched[kind].payload)
        for kind in ENTITY_ENDPOINTS
    }
    if channel == "stable":
        version = _version_evidence(fetched["changelogs"].payload)
    else:
        beta_version = fetched["beta_version"].payload
        if not isinstance(beta_version, dict) or not isinstance(
            beta_version.get("beta_version"), str
        ):
            raise DataSyncError("beta/version must report a beta_version string")
        version = {
            "status": "reported_by_beta_version_endpoint",
            "game_version": beta_version["beta_version"],
            "steam_build_id": None,
            "source_release": beta_version["beta_version"],
            "source_release_date": None,
            "changelog_title": None,
            "render_version": beta_version.get("render_version"),
        }
    endpoint_hashes = {
        endpoint: _sha256(result.body) for endpoint, result in fetched.items()
    }
    content_sha256 = _sha256(_canonical_json(endpoint_hashes))

    latest = _load_latest(output_root, channel)
    if (
        latest
        and latest.get("content_sha256") == content_sha256
        and latest.get("normalizer_version") == NORMALIZER_VERSION
        and not force
    ):
        relative_path = latest.get("snapshot_path")
        if isinstance(relative_path, str):
            existing = output_root / SOURCE_NAME / channel / relative_path
            manifest_path = existing / "manifest.json"
            if manifest_path.exists():
                manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
                return SnapshotResult(existing, manifest, False)

    instant = (now or datetime.now(timezone.utc)).astimezone(timezone.utc)
    timestamp = instant.strftime("%Y%m%dT%H%M%SZ")
    version_label = (version.get("game_version") or "unknown").lstrip("v")
    snapshot_name = f"{timestamp}-{content_sha256[:12]}"
    source_root = output_root / SOURCE_NAME / channel
    final_path = source_root / version_label / snapshot_name
    temporary_path = source_root / f".tmp-{uuid4().hex}"
    if final_path.exists():
        raise DataSyncError(f"snapshot path already exists: {final_path}")

    manifest = {
        "schema_version": 1,
        "normalizer_version": NORMALIZER_VERSION,
        "source": {
            "name": SOURCE_NAME,
            "channel": channel,
            "base_url": base_url.rstrip("/"),
            "developers_url": f"{base_url.rstrip('/')}/developers",
            "terms_url": "https://github.com/ptrlrd/spire-codex/blob/main/API_TERMS.md",
            "origin": "community API extracted by its maintainers from game files",
        },
        "retrieved_at": instant.isoformat().replace("+00:00", "Z"),
        "user_agent": user_agent,
        "content_sha256": content_sha256,
        "version_evidence": version,
        "endpoints": {
            endpoint: {
                "url": result.url,
                "sha256": endpoint_hashes[endpoint],
                "bytes": len(result.body),
                "records": len(result.payload) if isinstance(result.payload, list) else None,
                "headers": result.headers,
            }
            for endpoint, result in fetched.items()
        },
        "normalized": {
            kind: {
                "path": f"normalized/{kind}.json",
                "records": len(collections[kind]),
                "id_prefix": MODEL_PREFIXES[kind],
            }
            for kind in ENTITY_ENDPOINTS
        },
        "warnings": [
            "This is a community extraction, not an official Mega Crit database.",
            (
                "The hosted stable API provides no dataset-version response header; "
                "game version is inferred from changelog evidence."
                if channel == "stable"
                else "The beta version is reported by /api/beta/version; no Steam build id is exposed."
            ),
            "Do not promote values into the simulator without a build-pinned trace or mechanic test.",
            "Game data belongs to Mega Crit Games; review the upstream API terms before redistribution.",
        ],
    }

    try:
        raw_dir = temporary_path / "raw"
        normalized_dir = temporary_path / "normalized"
        raw_dir.mkdir(parents=True)
        normalized_dir.mkdir()
        for endpoint, result in fetched.items():
            (raw_dir / f"{endpoint}.json").write_bytes(result.body)
        for kind, records in collections.items():
            _write_json(
                normalized_dir / f"{kind}.json",
                _normalize_collection(kind, records, channel),
            )
        _write_json(temporary_path / "manifest.json", manifest)
        final_path.parent.mkdir(parents=True, exist_ok=True)
        temporary_path.rename(final_path)

        latest_payload = {
            "content_sha256": content_sha256,
            "normalizer_version": NORMALIZER_VERSION,
            "retrieved_at": manifest["retrieved_at"],
            "snapshot_path": str(final_path.relative_to(source_root)),
            "version_evidence": version,
        }
        latest_temp = source_root / f".latest-{uuid4().hex}.json"
        _write_json(latest_temp, latest_payload)
        latest_temp.replace(source_root / "latest.json")
    except Exception:
        shutil.rmtree(temporary_path, ignore_errors=True)
        raise

    return SnapshotResult(final_path, manifest, True)
