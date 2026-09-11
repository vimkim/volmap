"""Focused checks: python3 -m unittest discover -s tools -p 'test_check_overlay.py'."""
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import runpy
import socket
import tempfile
import threading
import unittest

CHECK = runpy.run_path(str(Path(__file__).with_name("check-overlay.py")))
CORPUS = Path(__file__).resolve().parents[1] / "fixtures/pgbuf-inspector/v1/corpus"


@contextmanager
def producer(frame):
    with tempfile.TemporaryDirectory() as directory:
        path = str(Path(directory) / "producer.sock")
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as server:
            server.bind(path)
            os.chmod(path, 0o600)
            server.listen(1)
            server.settimeout(3)

            def respond():
                connection, _ = server.accept()
                with connection:
                    connection.settimeout(3)
                    connection.recv(4096)
                    connection.sendall(frame)

            thread = threading.Thread(target=respond)
            thread.start()
            try:
                yield path
            finally:
                thread.join(timeout=4)


class OverlayCheckTests(unittest.TestCase):
    def test_parameter_uses_server_current_value_not_client_or_default(self):
        self.assertFalse(CHECK["parameter_value"](
            "[C*] enable_pgbuf_inspector=y (n)\n[S*] enable_pgbuf_inspector=n (y)\n"))
        self.assertTrue(CHECK["parameter_value"]("[S*] enable_pgbuf_inspector=y (n)\n"))

    def test_missing_hidden_parameter_is_unknown(self):
        with self.assertRaisesRegex(ValueError, "unknown"):
            CHECK["parameter_value"]("[C*] enable_pgbuf_inspector=y (n)\n")

    def test_pinned_greeting_over_private_socket(self):
        frames = (CORPUS / "exchanges/complete/stream.jsonl").read_bytes().splitlines()
        hello = next(frame for frame in frames if json.loads(frame)["type"] == "server_hello")
        with producer(hello + b"\n") as path:
            self.assertIn("same-UID protocol 1.", CHECK["check_producer"](path))

    def test_busy_producer_is_not_ready(self):
        with producer(b'{"type":"error","code":"busy"}\n') as path:
            with self.assertRaisesRegex(ValueError, "busy"):
                CHECK["check_producer"](path)

    def test_wrong_version_and_incomplete_greetings_fail(self):
        for frame in (b'{"type":"server_hello","protocol_major":2}\n', b'{"type":'):
            with self.subTest(frame=frame), producer(frame) as path:
                with self.assertRaises(ValueError):
                    CHECK["check_producer"](path)

    def test_missing_socket_does_not_imply_parameter_off(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "does not prove"):
                CHECK["check_producer"](str(Path(directory) / "missing.sock"))

    def test_insecure_socket_is_rejected_before_connect(self):
        with tempfile.TemporaryDirectory() as directory:
            path = str(Path(directory) / "producer.sock")
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as server:
                server.bind(path)
                os.chmod(path, 0o666)
                with self.assertRaisesRegex(ValueError, "0600"):
                    CHECK["check_producer"](path)

    def test_preview_requires_marker_and_successful_observation(self):
        class Handler(BaseHTTPRequestHandler):
            marker = True
            active = True
            posts = 0

            def log_message(self, *args):
                pass

            def reply(self, data):
                self.send_response(200)
                if self.marker:
                    self.send_header("X-Volmap-Preview", "synthetic-fixture")
                self.end_headers()
                self.wfile.write(json.dumps(data).encode())

            def do_GET(self):
                self.reply({"schema": "volmap.inspection", "schema_version": 1,
                            "document_type": "resource", "snapshot": {"generation": "7"}})

            def do_POST(self):
                Handler.posts += 1
                self.rfile.read(int(self.headers["Content-Length"]))
                self.reply({"schema": "volmap.runtime.page-buffer", "schema_version": 1,
                            "capability": {"source": "cubrid-page-buffer-observation",
                                           "state": "active" if self.active else "unavailable",
                                           "verification": "verified", "reason": "test-source"},
                            "evaluated_count": 1, "capture": {"sequence": "1"}})

        with ThreadingHTTPServer(("127.0.0.1", 0), Handler) as server:
            thread = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.01})
            thread.start()
            url = f"http://127.0.0.1:{server.server_port}"
            try:
                self.assertIn("observation received", CHECK["check_preview"](url))
                Handler.active = False
                with self.assertRaisesRegex(ValueError, "unavailable"):
                    CHECK["check_preview"](url)
                Handler.marker = False
                with self.assertRaisesRegex(ValueError, "not the synthetic"):
                    CHECK["check_preview"](url)
                self.assertEqual(Handler.posts, 2)
            finally:
                server.shutdown()
                thread.join()


if __name__ == "__main__":
    unittest.main()
