#!/usr/bin/env python3
"""Enable the local STS2 combat oracle mod in STS2's settings.save."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import time
from datetime import datetime, timezone
from pathlib import Path


DEFAULT_BASE = (
    Path.home()
    / "Library"
    / "Application Support"
    / "SlayTheSpire2"
)

BRIDGE_MOD = {
    "id": "sls2_combat_oracle",
    "source": "mods_directory",
    "is_enabled": True,
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--settings",
        type=Path,
        default=None,
        help="Path to STS2 settings.save. Defaults to the newest Steam settings file, then default/1.",
    )
    parser.add_argument(
        "--disable",
        action="store_true",
        help="Mark the bridge mod disabled instead of enabled.",
    )
    parser.add_argument(
        "--mark-ea-disclaimer-seen",
        action="store_true",
        help="Set seen_ea_disclaimer=true to avoid startup modal conflicts.",
    )
    parser.add_argument(
        "--no-backup",
        action="store_true",
        help="Do not create a timestamped backup before writing.",
    )
    return parser.parse_args()


def find_settings_path(explicit: Path | None) -> Path:
    if explicit is not None:
        return explicit

    candidates = list((DEFAULT_BASE / "steam").glob("*/settings.save"))
    candidates.append(DEFAULT_BASE / "default" / "1" / "settings.save")
    existing = [path for path in candidates if path.exists()]
    if not existing:
        raise SystemExit("settings file not found")
    return max(existing, key=lambda path: path.stat().st_mtime)


def main() -> int:
    args = parse_args()
    settings_path = find_settings_path(args.settings)
    if not settings_path.exists():
        raise SystemExit(f"settings file not found: {settings_path}")

    data = json.loads(settings_path.read_text())
    if args.mark_ea_disclaimer_seen:
        data["seen_ea_disclaimer"] = True

    mod_settings = data.get("mod_settings") or {}
    mod_settings["mods_enabled"] = not args.disable

    mod_list = list(mod_settings.get("mod_list") or [])
    mod_list = [
        item
        for item in mod_list
        if not (
            item.get("id") == BRIDGE_MOD["id"]
            and item.get("source") == BRIDGE_MOD["source"]
        )
    ]
    mod_entry = dict(BRIDGE_MOD)
    mod_entry["is_enabled"] = not args.disable
    mod_list.append(mod_entry)
    mod_settings["mod_list"] = mod_list
    data["mod_settings"] = mod_settings

    if not args.no_backup:
        timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        backup = settings_path.with_name(f"{settings_path.name}.{timestamp}.bak")
        shutil.copy2(settings_path, backup)
        print(f"backup: {backup}")

    settings_path.write_text(json.dumps(data, indent=2, sort_keys=False) + "\n")
    sync_steam_cache_metadata(settings_path)
    print(f"updated: {settings_path}")
    return 0


def sync_steam_cache_metadata(settings_path: Path) -> None:
    parts = settings_path.parts
    if "Steam" not in parts or "userdata" not in parts or "remote" not in parts:
        return

    remote_index = parts.index("remote")
    userdata_root = Path(*parts[:remote_index])
    relative_name = str(Path(*parts[remote_index + 1 :]))
    if relative_name != "settings.save":
        return

    cache_path = userdata_root / "remotecache.vdf"
    if not cache_path.exists():
        return

    payload = settings_path.read_bytes()
    size = str(len(payload))
    timestamp = str(int(time.time()))
    sha = hashlib.sha1(payload).hexdigest()
    text = cache_path.read_text()

    pattern = re.compile(r'("settings\.save"\s*\{\s*)(.*?)(\n\s*\})', re.DOTALL)
    match = pattern.search(text)
    if not match:
        return

    block = match.group(2)
    replacements = {
        "size": size,
        "localtime": timestamp,
        "time": timestamp,
        "remotetime": timestamp,
        "sha": sha,
    }
    for key, value in replacements.items():
        block = re.sub(
            rf'("{key}"\s*)"[^\"]*"',
            rf'\1"{value}"',
            block,
            count=1,
        )

    cache_path.write_text(text[: match.start(2)] + block + text[match.end(2) :])
    print(f"updated cache: {cache_path}")


if __name__ == "__main__":
    raise SystemExit(main())
