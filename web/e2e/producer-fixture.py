#!/usr/bin/env python3
"""Script the pinned producer transcript over a real authenticated test socket."""
import json
import os
from pathlib import Path
import socket
import signal
import sys

socket_path, volume_path, corpus_path = sys.argv[1:]
frames = [json.loads(line) for line in Path(corpus_path).read_text().splitlines()]
hello = next(frame for frame in frames if frame["type"] == "server_hello")
# create-smoke-fixture.rs pins both creation values to zero. Only OS identity
# varies between runs; no production authentication or decoder is bypassed.
stamp = os.stat(volume_path)
hello.update(database_creation="0", volumes=[dict(volid=0, volume_creation="0",
    device=str(stamp.st_dev), inode=str(stamp.st_ino))])
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(socket_path)
os.chmod(socket_path, 0o600)
server.listen(1)
print("ready", flush=True)
dense = os.environ.get("VOLMAP_BROWSER_DENSE") == "1"
sequence = 0
active_connection = None

def restart(_signal, _frame):
    global sequence
    sequence = 0
    hello["incarnation"] = os.urandom(16).hex()
    if active_connection is not None:
        active_connection.shutdown(socket.SHUT_RDWR)

signal.signal(signal.SIGHUP, restart)
while True:
    connection, _ = server.accept()
    active_connection = connection
    try:
        with connection, connection.makefile("rb") as reader:
            if not reader.readline():
                continue
            connection.sendall((json.dumps(hello) + "\n").encode())
            while reader.readline():
                sequence += 1
                for source in frames:
                    if source["type"] not in ("scan_header", "page", "scan_footer"):
                        continue
                    frame = dict(source, scan_seq=str(sequence), incarnation=hello["incarnation"])
                    if dense and frame["type"] == "page":
                        # Every physical page in the 192-sector fixture is resident;
                        # all LRU zones change per capture. No UI/API interception.
                        rows = []
                        for pageid in range(12288):
                            row = dict(type="page", incarnation=hello["incarnation"], scan_seq=str(sequence),
                                volid=0, pageid=pageid, lru_zone=f"lru{1 + sequence % 3}",
                                lru_list_kind="private" if pageid % 2 else "shared", lru_list_index=pageid % 2)
                            rows.append(json.dumps(row, separators=(",", ":")) + "\n")
                        connection.sendall("".join(rows).encode())
                        continue
                    if frame["type"] == "page":
                        frame["pageid"] = 10
                    if dense and frame["type"] == "scan_footer":
                        frame.update(record_count=12288, visited_slots=12288)
                    # Ordinary JSON retaining real wire framing and authentication.
                    connection.sendall((json.dumps(frame) + "\n").encode())
    except (BrokenPipeError, ConnectionResetError):
        # Scope/visibility changes cancel the last observer mid-scan.
        # A fixture connection ends; the producer remains available.
        pass

    active_connection = None
