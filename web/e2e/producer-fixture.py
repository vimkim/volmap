#!/usr/bin/env python3
"""Script the pinned producer transcript over a real authenticated test socket."""
import json
import os
from pathlib import Path
import socket
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
sequence = 0
while True:
    connection, _ = server.accept()
    with connection, connection.makefile("rb") as reader:
        if not reader.readline():
            continue
        connection.sendall((json.dumps(hello) + "\n").encode())
        while reader.readline():
            sequence += 1
            for source in frames:
                if source["type"] not in ("scan_header", "page", "scan_footer"):
                    continue
                frame = dict(source, scan_seq=str(sequence))
                if frame["type"] == "page":
                    frame["pageid"] = 10
                # Deliberate coalescing/format independence: ordinary JSON,
                # retaining the pinned semantic values and real wire framing.
                connection.sendall((json.dumps(frame) + "\n").encode())
