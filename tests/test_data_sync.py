from __future__ import annotations

import json
import threading
import unittest
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from tempfile import TemporaryDirectory

from sls2_combat_solver.data_sync import DataSyncError, fetch_spire_codex_snapshot


RESPONSES = {
    "/api/changelogs": [
        {
            "build_id": "23811903",
            "date": "2026-06-18",
            "game_version": "1.2.0",
            "tag": "1.2.0",
            "title": "Slay the Spire 2 v0.107.1: balance pass",
        }
    ],
    "/api/cards": [
        {
            "id": "STRIKE_SILENT",
            "name": "Strike",
            "cost": 1,
            "type": "Attack",
            "rarity": "Basic",
            "damage": 6,
        }
    ],
    "/api/relics": [
        {
            "id": "RING_OF_THE_SNAKE",
            "name": "Ring of the Snake",
            "description": "Draw more cards.",
            "rarity": "Starter Relic",
        }
    ],
    "/api/monsters": [
        {
            "id": "NIBBIT",
            "name": "Nibbit",
            "type": "Normal",
            "moves": [],
            "min_hp": 42,
        }
    ],
}

BETA_RESPONSES = {
    "/api/beta/version": {
        "beta_version": "v0.110.0",
        "render_version": "v0.110.0",
    },
    "/api/beta/diff": {"beta_version": "v0.110.0", "types": {}},
    "/api/cards?channel=beta": RESPONSES["/api/cards"],
    "/api/relics?channel=beta": RESPONSES["/api/relics"],
    "/api/monsters?channel=beta": RESPONSES["/api/monsters"],
}


class DataSyncTests(unittest.TestCase):
    def setUp(self) -> None:
        _DataHandler.responses = RESPONSES
        _DataHandler.paths = []
        _DataHandler.user_agents = []
        self.server = HTTPServer(("127.0.0.1", 0), _DataHandler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.addCleanup(self.server.shutdown)
        self.addCleanup(self.server.server_close)
        self.base_url = f"http://127.0.0.1:{self.server.server_port}"

    def test_snapshot_saves_raw_normalized_and_provenance(self) -> None:
        with TemporaryDirectory() as temporary:
            result = fetch_spire_codex_snapshot(
                Path(temporary),
                base_url=self.base_url,
                user_agent="test-crawler/1.0",
                delay_seconds=0,
                now=datetime(2026, 8, 4, 2, 3, 4, tzinfo=timezone.utc),
            )

            self.assertTrue(result.created)
            self.assertEqual(
                _DataHandler.paths,
                [
                    "/api/changelogs",
                    "/api/cards",
                    "/api/relics",
                    "/api/monsters",
                ],
            )
            self.assertEqual(set(_DataHandler.user_agents), {"test-crawler/1.0"})
            self.assertEqual(
                result.manifest["version_evidence"]["game_version"], "v0.107.1"
            )
            self.assertEqual(
                result.manifest["version_evidence"]["steam_build_id"], "23811903"
            )
            self.assertTrue((result.path / "raw" / "cards.json").exists())
            cards = json.loads(
                (result.path / "normalized" / "cards.json").read_text(encoding="utf-8")
            )
            self.assertEqual(cards[0]["model_id"], "CARD.STRIKE_SILENT")
            self.assertEqual(cards[0]["source_id"], "STRIKE_SILENT")
            latest = json.loads(
                (Path(temporary) / "spire_codex" / "stable" / "latest.json").read_text(
                    encoding="utf-8"
                )
            )
            self.assertEqual(latest["content_sha256"], result.manifest["content_sha256"])
            self.assertEqual(latest["normalizer_version"], 1)

    def test_identical_content_reuses_latest_snapshot(self) -> None:
        with TemporaryDirectory() as temporary:
            output_root = Path(temporary)
            first = fetch_spire_codex_snapshot(
                output_root, base_url=self.base_url, delay_seconds=0
            )
            second = fetch_spire_codex_snapshot(
                output_root, base_url=self.base_url, delay_seconds=0
            )

            self.assertTrue(first.created)
            self.assertFalse(second.created)
            self.assertEqual(second.path, first.path)

    def test_schema_drift_is_rejected_before_writing(self) -> None:
        broken = dict(RESPONSES)
        broken["/api/cards"] = [{"id": "BROKEN", "name": "Broken"}]
        _DataHandler.responses = broken
        with TemporaryDirectory() as temporary:
            output_root = Path(temporary)
            with self.assertRaisesRegex(DataSyncError, "missing fields"):
                fetch_spire_codex_snapshot(
                    output_root, base_url=self.base_url, delay_seconds=0
                )
            self.assertFalse(
                (output_root / "spire_codex" / "stable" / "latest.json").exists()
            )

    def test_beta_channel_is_versioned_and_kept_separate(self) -> None:
        _DataHandler.responses = BETA_RESPONSES
        with TemporaryDirectory() as temporary:
            result = fetch_spire_codex_snapshot(
                Path(temporary),
                base_url=self.base_url,
                delay_seconds=0,
                channel="beta",
            )

            self.assertTrue(result.created)
            self.assertEqual(result.manifest["source"]["channel"], "beta")
            self.assertEqual(
                result.manifest["version_evidence"]["game_version"], "v0.110.0"
            )
            self.assertIn("/spire_codex/beta/0.110.0/", result.path.as_posix())
            cards = json.loads(
                (result.path / "normalized" / "cards.json").read_text(encoding="utf-8")
            )
            self.assertEqual(cards[0]["source"], "spire_codex:beta")


class _DataHandler(BaseHTTPRequestHandler):
    responses: dict[str, object] = RESPONSES
    paths: list[str] = []
    user_agents: list[str] = []

    def do_GET(self) -> None:
        type(self).paths.append(self.path)
        type(self).user_agents.append(self.headers.get("User-Agent", ""))
        payload = type(self).responses.get(self.path)
        if payload is None:
            self.send_error(404)
            return
        body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("ETag", '"fixture"')
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        return


if __name__ == "__main__":
    unittest.main()
