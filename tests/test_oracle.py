from __future__ import annotations

import threading
import unittest
import json
from http.server import BaseHTTPRequestHandler, HTTPServer

from sls2_combat_solver.oracle import HttpGameOracle, OracleError


class OracleClientTests(unittest.TestCase):
    def test_http_error_includes_oracle_error_body(self) -> None:
        server = HTTPServer(("127.0.0.1", 0), _FailingOracleHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.shutdown)
        self.addCleanup(server.server_close)

        oracle = HttpGameOracle(f"http://127.0.0.1:{server.server_port}")

        with self.assertRaisesRegex(
            OracleError,
            "HTTP 501: Branchable /step requires combat-state clone/restore",
        ):
            oracle.step({"player": {"hp": 1}}, {"id": "end_turn"})

    def test_live_step_sends_explicit_mutation_ack(self) -> None:
        _LiveStepHandler.last_request = None
        server = HTTPServer(("127.0.0.1", 0), _LiveStepHandler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.shutdown)
        self.addCleanup(server.server_close)

        oracle = HttpGameOracle(f"http://127.0.0.1:{server.server_port}")

        state = oracle.live_step(
            {"id": "play:0:enemy_0", "type": "card"},
            allow_live_mutation=True,
            timeout_milliseconds=1234,
        )

        self.assertEqual(state["id"], "after")
        self.assertIsNotNone(_LiveStepHandler.last_request)
        self.assertEqual(_LiveStepHandler.last_request["action"]["id"], "play:0:enemy_0")
        self.assertTrue(_LiveStepHandler.last_request["allow_live_mutation"])
        self.assertEqual(_LiveStepHandler.last_request["timeout_milliseconds"], 1234)


class _FailingOracleHandler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:
        body = b'{"error":"Branchable /step requires combat-state clone/restore"}'
        self.send_response(501)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        return


class _LiveStepHandler(BaseHTTPRequestHandler):
    last_request: dict | None = None

    def do_POST(self) -> None:
        length = int(self.headers["Content-Length"])
        body = self.rfile.read(length).decode("utf-8")
        type(self).last_request = json.loads(body)
        response = b'{"state":{"id":"after"}}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(response)))
        self.end_headers()
        self.wfile.write(response)

    def log_message(self, format: str, *args: object) -> None:
        return


if __name__ == "__main__":
    unittest.main()
