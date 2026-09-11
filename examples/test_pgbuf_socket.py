"""Focused diagnostic-client checks using the pinned producer transcript."""
import json
from pathlib import Path
import socket
import unittest

from pgbuf_socket import Frames, ProtocolError, scan


class ScanTest(unittest.TestCase):
    def setUp(self):
        corpus = Path(__file__).resolve().parents[1] / "fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
        self.transcript = [json.loads(line) for line in corpus.read_text().splitlines()]

    def run_scan(self, frames, pageid=7):
        client, producer = socket.socketpair()
        with client, producer:
            producer.sendall(b"".join((json.dumps(frame) + "\n").encode() for frame in frames))
            producer.shutdown(socket.SHUT_WR)
            return scan(client, self.transcript[1], 0, pageid, Frames(client))

    def test_complete_capture_exposes_selected_fields(self):
        result = self.run_scan(self.transcript[3:])
        self.assertEqual(result["status"], "observed")
        self.assertEqual(result["observation"]["fix_count"], 2)

    def test_partial_omission_is_not_absence(self):
        self.transcript[-1]["truncated"] = True
        result = self.run_scan(self.transcript[3:], pageid=99)
        self.assertEqual(result["status"], "not-observed")
        self.assertIsNone(result["observation"])

    def test_duplicate_target_is_ambiguous(self):
        frames = self.transcript[3:]
        frames.insert(2, frames[1].copy())
        frames[-1]["record_count"] = 2
        result = self.run_scan(frames)
        self.assertEqual(result["status"], "ambiguous")
        self.assertIsNone(result["observation"])

    def test_missing_footer_discards_observation(self):
        with self.assertRaises(ProtocolError):
            self.run_scan(self.transcript[3:-1])

    def test_bad_count_discards_observation(self):
        self.transcript[-1]["record_count"] = 0
        with self.assertRaises(ProtocolError):
            self.run_scan(self.transcript[3:])


if __name__ == "__main__":
    unittest.main()
