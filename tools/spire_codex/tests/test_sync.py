from __future__ import annotations

import json
import threading
import unittest
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from tempfile import TemporaryDirectory

import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from build_catalog import CatalogBuildError, build_catalog
from sync import DataSyncError, fetch_spire_codex_snapshot


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
    "/api/characters": [
        {
            "id": "SILENT",
            "name": "The Silent",
            "max_energy": 3,
            "starting_hp": 70,
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
    "/api/potions": [
        {
            "id": "BLOCK_POTION",
            "name": "Block Potion",
            "description": "Gain Block.",
            "rarity": "Common",
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
    "/api/powers": [
        {
            "id": "WEAK_POWER",
            "name": "Weak",
            "type": "Debuff",
            "stack_type": "Duration",
        }
    ],
    "/api/encounters": [
        {
            "id": "NIBBITS_WEAK",
            "name": "Nibbit",
            "monsters": ["NIBBIT"],
            "room_type": "Monster",
        }
    ],
    "/api/ascensions": [
        {
            "id": "ASCENSION_0",
            "name": "Ascension 0",
            "level": 0,
            "description": "Base difficulty.",
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
    "/api/characters?channel=beta": RESPONSES["/api/characters"],
    "/api/relics?channel=beta": RESPONSES["/api/relics"],
    "/api/potions?channel=beta": RESPONSES["/api/potions"],
    "/api/monsters?channel=beta": RESPONSES["/api/monsters"],
    "/api/powers?channel=beta": RESPONSES["/api/powers"],
    "/api/encounters?channel=beta": RESPONSES["/api/encounters"],
    "/api/ascensions?channel=beta": RESPONSES["/api/ascensions"],
}


class DataSyncTests(unittest.TestCase):
    def setUp(self) -> None:
        _DataHandler.responses = RESPONSES
        _DataHandler.paths = []
        _DataHandler.user_agents = []
        _DataHandler.rate_limit_once = False
        _DataHandler.rate_limit_served = False
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
                    "/api/characters",
                    "/api/relics",
                    "/api/potions",
                    "/api/monsters",
                    "/api/powers",
                    "/api/encounters",
                    "/api/ascensions",
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
            for endpoint in (
                "characters",
                "potions",
                "powers",
                "encounters",
                "ascensions",
            ):
                self.assertTrue((result.path / "raw" / f"{endpoint}.json").exists())
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

    def test_force_never_overwrites_an_existing_snapshot(self) -> None:
        instant = datetime(2026, 8, 4, 2, 3, 4, tzinfo=timezone.utc)
        with TemporaryDirectory() as temporary:
            output_root = Path(temporary)
            fetch_spire_codex_snapshot(
                output_root,
                base_url=self.base_url,
                delay_seconds=0,
                now=instant,
            )
            with self.assertRaisesRegex(DataSyncError, "already exists"):
                fetch_spire_codex_snapshot(
                    output_root,
                    base_url=self.base_url,
                    delay_seconds=0,
                    force=True,
                    now=instant,
                )

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

    def test_duplicate_ids_are_rejected_before_writing(self) -> None:
        broken = dict(RESPONSES)
        broken["/api/cards"] = [RESPONSES["/api/cards"][0]] * 2
        _DataHandler.responses = broken
        with TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(DataSyncError, "duplicate id"):
                fetch_spire_codex_snapshot(
                    Path(temporary), base_url=self.base_url, delay_seconds=0
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

    def test_rate_limit_is_retried(self) -> None:
        _DataHandler.rate_limit_once = True
        with TemporaryDirectory() as temporary:
            result = fetch_spire_codex_snapshot(
                Path(temporary),
                base_url=self.base_url,
                delay_seconds=0,
                attempts=2,
            )
            self.assertTrue(result.created)
            self.assertEqual(_DataHandler.paths.count("/api/cards"), 2)

    def test_catalog_build_is_reviewed_and_deterministic(self) -> None:
        reviewed = {
            "schema_version": 1,
            "source": {
                "name": "spire_codex",
                "channel": "stable",
                "game_version": "review-time",
                "content_sha256": "review-time",
                "retrieved_at": "review-time",
            },
            "rng_profiles": {
                "isolated_combat_xoshiro_v1": {
                    "algorithm": "xoshiro256_star_star_v1",
                    "stream_derivation": "numeric_seed_domain_v1",
                }
            },
            "characters": {"CHARACTER.SILENT": {"max_energy": 3}},
            "cards": {"CARD.STRIKE_SILENT": {"cost": 1}},
            "relics": {"RELIC.RING_OF_THE_SNAKE": {"combat_effect": {"type": "inert"}}},
            "powers": {
                "POWER.WEAK_POWER": {
                    "source_id": "WEAK_POWER",
                    "stack_behavior": "duration",
                }
            },
            "monsters": {
                "MONSTER.NIBBIT": {
                    "hp": {"min": 42, "max": 46},
                    "ascension_hp": {"min": 44, "max": 48},
                    "opening_move": "BUTT_MOVE",
                    "moves": {"BUTT_MOVE": {"next_move": "BUTT_MOVE"}},
                }
            },
            "encounters": {"ENCOUNTER.NIBBITS_WEAK": {"enemies": ["MONSTER.NIBBIT"]}},
            "combat_modifiers": {
                "weak": {"numerator": 3, "denominator": 4},
                "shrink": {"numerator": 7, "denominator": 10},
                "vulnerable": {"numerator": 3, "denominator": 2},
            },
        }
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            snapshot = fetch_spire_codex_snapshot(
                root,
                base_url=self.base_url,
                delay_seconds=0,
                now=datetime(2026, 8, 4, 2, 3, 4, tzinfo=timezone.utc),
            ).path
            reviewed_path = root / "reviewed.json"
            reviewed_path.write_text(json.dumps(reviewed), encoding="utf-8")
            first = root / "first.json"
            second = root / "second.json"
            first_result = build_catalog(snapshot, reviewed_path, first)
            second_result = build_catalog(snapshot, reviewed_path, second)
            self.assertEqual(first.read_bytes(), second.read_bytes())
            self.assertEqual(first_result["sha256"], second_result["sha256"])

            reviewed["cards"]["CARD.NOT_IN_SNAPSHOT"] = {"cost": 1}
            reviewed_path.write_text(json.dumps(reviewed), encoding="utf-8")
            with self.assertRaisesRegex(CatalogBuildError, "absent from snapshot"):
                build_catalog(snapshot, reviewed_path, root / "rejected.json")


class _DataHandler(BaseHTTPRequestHandler):
    responses: dict[str, object] = RESPONSES
    paths: list[str] = []
    user_agents: list[str] = []
    rate_limit_once = False
    rate_limit_served = False

    def do_GET(self) -> None:
        type(self).paths.append(self.path)
        type(self).user_agents.append(self.headers.get("User-Agent", ""))
        if (
            self.path == "/api/cards"
            and type(self).rate_limit_once
            and not type(self).rate_limit_served
        ):
            type(self).rate_limit_served = True
            self.send_response(429)
            self.send_header("Retry-After", "0")
            self.end_headers()
            return
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
