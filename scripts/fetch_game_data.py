from __future__ import annotations

import argparse
import json
import time
from pathlib import Path

from sls2_combat_solver.data_sync import (
    DEFAULT_BASE_URL,
    DEFAULT_USER_AGENT,
    DataSyncError,
    fetch_spire_codex_snapshot,
)


PROJECT_ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Save an immutable, provenance-rich Spire Codex data snapshot."
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=PROJECT_ROOT / "data" / "upstream",
    )
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--user-agent", default=DEFAULT_USER_AGENT)
    parser.add_argument("--timeout-seconds", type=float, default=30.0)
    parser.add_argument(
        "--delay-seconds",
        type=float,
        default=1.05,
        help="Delay between requests; default stays within the published 60/min limit.",
    )
    parser.add_argument("--attempts", type=int, default=3)
    parser.add_argument(
        "--channel",
        choices=("stable", "beta", "both"),
        default="both",
        help="Snapshot stable, beta, or both independently (default: both).",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Write a new timestamped snapshot even when content hashes are unchanged.",
    )
    args = parser.parse_args()

    try:
        channels = ("stable", "beta") if args.channel == "both" else (args.channel,)
        results = []
        for index, channel in enumerate(channels):
            if index and args.delay_seconds:
                time.sleep(args.delay_seconds)
            results.append(
                fetch_spire_codex_snapshot(
                    args.output_root,
                    base_url=args.base_url,
                    user_agent=args.user_agent,
                    timeout_seconds=args.timeout_seconds,
                    delay_seconds=args.delay_seconds,
                    attempts=args.attempts,
                    force=args.force,
                    channel=channel,
                )
            )
    except (DataSyncError, OSError, ValueError) as exc:
        parser.exit(1, f"data fetch failed: {exc}\n")

    summary = []
    for result in results:
        summary.append(
            {
                "channel": result.manifest["source"]["channel"],
                "created": result.created,
                "snapshot_path": str(result.path.resolve()),
                "content_sha256": result.manifest["content_sha256"],
                "version_evidence": result.manifest["version_evidence"],
                "record_counts": {
                    kind: metadata["records"]
                    for kind, metadata in result.manifest["normalized"].items()
                },
            }
        )
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
