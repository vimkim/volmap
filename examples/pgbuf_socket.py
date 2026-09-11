#!/usr/bin/env python3
"""One-shot Linux diagnostic client for the CUBRID v1 inspector socket."""

import argparse
import json
import os
import socket
import stat
import struct
import sys
import time


class ProtocolError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise ProtocolError(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON field: {key}")
        result[key] = value
    return result


def reject_constant(value):
    raise ProtocolError(f"non-finite JSON number: {value}")


def check_depth(value, depth=1):
    if isinstance(value, (dict, list)):
        require(depth <= 16, "JSON nesting exceeds 16")
        for child in value.values() if isinstance(value, dict) else value:
            check_depth(child, depth + 1)


class Frames:
    def __init__(self, connection):
        self.connection = connection
        self.buffer = bytearray()
        self.bytes = 0

    def read(self, limit, deadline):
        while True:
            if time.monotonic() >= deadline:
                raise TimeoutError("exchange deadline exceeded")
            end = self.buffer.find(b"\n")
            if end >= 0:
                require(end + 1 <= limit, "frame exceeds byte limit")
                line = bytes(self.buffer[:end + 1])
                del self.buffer[:end + 1]
                self.bytes += len(line)
                require(self.bytes <= 64 * 1024 * 1024, "exchange exceeds 64 MiB")
                value = json.loads(line, object_pairs_hook=unique_object,
                                   parse_constant=reject_constant)
                require(isinstance(value, dict), "expected JSON object")
                check_depth(value)
                require(value.get("type") != "error", f"producer refusal: {value}")
                return value
            require(len(self.buffer) < limit, "unterminated or oversized frame")
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("exchange deadline exceeded")
            self.connection.settimeout(remaining)
            chunk = self.connection.recv(min(4096, limit - len(self.buffer)))
            require(bool(chunk), "stream ended before complete frame/footer")
            self.buffer.extend(chunk)


def scan(connection, hello, volid, pageid, frames):
    deadline = time.monotonic() + 2
    connection.settimeout(2)
    connection.sendall((json.dumps({"type": "scan_request",
                                   "incarnation": hello["incarnation"]}) + "\n").encode())
    frames.bytes = 0
    header = frames.read(4096, deadline)
    require(header.get("type") == "scan_header", "expected scan_header")
    sequence = header.get("scan_seq")
    require(isinstance(sequence, str) and sequence.isdecimal() and int(sequence) > 0,
            "invalid scan sequence")
    require(header.get("incarnation") == hello["incarnation"], "incarnation changed")
    count, matches = 0, []
    while True:
        frame = frames.read(4096, deadline)
        require(frame.get("incarnation") == hello["incarnation"]
                and frame.get("scan_seq") == sequence, "capture identity changed")
        if frame.get("type") == "scan_footer":
            require(type(frame.get("record_count")) is int
                    and frame["record_count"] == count, "record count mismatch")
            require(type(frame.get("visited_slots")) is int
                    and count <= frame["visited_slots"] <= 65536, "invalid visited slots")
            require(type(frame.get("truncated")) is bool, "missing coverage flag")
            break
        require(frame.get("type") == "page", "unexpected scan frame")
        count += 1
        require(count <= 65536, "record limit exceeded")
        if frame.get("volid") == volid and frame.get("pageid") == pageid:
            if len(matches) < 2:
                matches.append(frame)
    # Publish only after a valid footer. Two matching records are ambiguous.
    return {"server": hello, "requested_vpid": {"volid": volid, "pageid": pageid},
            "status": "observed" if len(matches) == 1 else
                      "ambiguous" if matches else "not-observed",
            "observation": matches[0] if len(matches) == 1 else None,
            "scan_header": header, "scan_footer": frame,
            "identity_scope": "same-UID socket peer; server-reported database identity only",
            "limitation": "Non-atomic diagnostic scan; no page contents or BCB addresses. "
                          "Omission is not proof of current non-residency."}


def query(path, volid, pageid):
    require(hasattr(socket, "SO_PEERCRED"), "this example requires Linux SO_PEERCRED")
    for entry, mode, kind in [(os.path.dirname(path), 0o700, stat.S_ISDIR),
                              (path, 0o600, stat.S_ISSOCK)]:
        metadata = os.lstat(entry)
        require(kind(metadata.st_mode) and metadata.st_uid == os.geteuid()
                and stat.S_IMODE(metadata.st_mode) == mode,
                f"expected same-owner protected endpoint: {entry}")
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        deadline = time.monotonic() + .5
        connection.settimeout(.5)
        connection.connect(path)
        _, uid, _ = struct.unpack("3i", connection.getsockopt(
            socket.SOL_SOCKET, socket.SO_PEERCRED, struct.calcsize("3i")))
        require(uid == os.geteuid(), "peer UID mismatch")
        connection.sendall(b'{"type":"client_hello","supported_majors":[1]}\n')
        frames = Frames(connection)
        hello = frames.read(65536, deadline)
        require(hello.get("type") == "server_hello" and hello.get("protocol_major") == 1,
                "incompatible server hello")
        require(isinstance(hello.get("incarnation"), str) and hello["incarnation"],
                "missing incarnation")
        return scan(connection, hello, volid, pageid, frames)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", required=True, help="explicit inspector Unix socket path")
    parser.add_argument("volid", type=int)
    parser.add_argument("pageid", type=int)
    args = parser.parse_args()
    if not (0 <= args.volid <= 32767 and 0 <= args.pageid <= 2147483647):
        parser.error("VPID must have nonnegative int16 volid and int32 pageid")
    try:
        print(json.dumps(query(os.path.abspath(args.socket), args.volid, args.pageid), indent=2))
    except (OSError, ValueError, RecursionError) as error:
        print(f"pgbuf example: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
