"""Check the synthetic preview and a CUBRID producer without changing configuration."""
import argparse
import http.client
import json
import os
from pathlib import Path
import re
import socket
import stat
import struct
import subprocess
import tempfile
import time
from urllib.parse import urlsplit

TIMEOUT = 3
MAX_JSON = 65536


def object_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON field")
        result[key] = value
    return result


def decode(raw):
    value = json.loads(raw, object_pairs_hook=object_pairs)
    if not isinstance(value, dict):
        raise ValueError("expected a JSON object")
    return value


def check_preview(url):
    """Use the preview marker before issuing any synthetic observation request."""
    target = urlsplit(url)
    if (target.scheme != "http" or not target.hostname or target.username
            or target.password or target.path not in ("", "/")
            or target.query or target.fragment):
        raise ValueError("preview URL must be an HTTP origin, e.g. http://192.168.4.2:7777")

    def request(path, body=None):
        connection = http.client.HTTPConnection(target.hostname, target.port, timeout=TIMEOUT)
        try:
            headers = {"Origin": f"http://{target.netloc}"}
            if body is not None:
                headers["Content-Type"] = "application/json"
            connection.request("GET" if body is None else "POST", path,
                               body=None if body is None else json.dumps(body), headers=headers)
            response = connection.getresponse()
            if response.status != 200:
                raise ValueError(f"preview returned HTTP {response.status}")
            if response.getheader("X-Volmap-Preview") != "synthetic-fixture":
                raise ValueError("endpoint is not the synthetic overlay preview")
            raw = response.read(MAX_JSON + 1)
            if len(raw) > MAX_JSON:
                raise ValueError("preview response exceeds 64 KiB")
            return decode(raw)
        finally:
            connection.close()

    session = request("/api/v1/session")
    if (session.get("schema") != "volmap.inspection" or session.get("schema_version") != 1
            or session.get("document_type") != "resource"):
        raise ValueError("unrecognized preview session")
    generation = session.get("snapshot", {}).get("generation")
    if not isinstance(generation, str) or not generation.isdecimal():
        raise ValueError("preview has no live generation")
    batch = request("/api/v1/runtime/page-buffer/observe", {
        "pages": [{"volid": 0, "pageid": 10}], "epoch": "1",
        "generation": generation, "retry": True,
    })
    capability = batch.get("capability", {})
    if (batch.get("schema") != "volmap.runtime.page-buffer" or batch.get("schema_version") != 1
            or capability.get("source") != "cubrid-page-buffer-observation"
            or capability.get("state") != "active" or capability.get("verification") != "verified"
            or batch.get("evaluated_count") != 1 or not batch.get("capture")):
        raise ValueError(f"synthetic observations unavailable: {capability.get('reason', 'invalid response')}")
    return "available; synthetic observation received (no CUBRID configuration required)"


def parameter_value(output):
    # New-style paramdump marks server values [S ] / [S*]. Never accept a
    # client value, a config-file value, or the default in parentheses.
    values = re.findall(r"^\[S[ *]\]\s+enable_pgbuf_inspector\s*=\s*(\w+)\b",
                        output, re.MULTILINE)
    if len(values) != 1:
        raise ValueError("server value not reported; parameter support/value is unknown")
    value = values[0].lower()
    if value in ("y", "yes", "true", "1", "on"):
        return True
    if value in ("n", "no", "false", "0", "off"):
        return False
    raise ValueError("unrecognized server parameter value")


def check_parameter(database):
    if not database or database.startswith("-"):
        raise ValueError("supply a database name")
    # PRM_HIDDEN is 0x8 in the producer's CUBRID source. Ordinary paramdump
    # excludes this startup-only parameter. Keep utility error logs temporary.
    with tempfile.TemporaryDirectory(prefix="volmap-paramdump-") as directory:
        result = subprocess.run(
            ["cubrid", "paramdump", "-C", "--dump-flag", "8", database],
            cwd=directory, capture_output=True, text=True, timeout=10,
            env={**os.environ, "LC_ALL": "C"},
        )
    if result.returncode:
        raise ValueError(f"paramdump failed (exit {result.returncode}); check the database is running "
                         "and cubrid on PATH supports --dump-flag")
    if not parameter_value(result.stdout):
        raise ValueError(f"{database}: enable_pgbuf_inspector=n in the running server. "
                         "For real overlays, set enable_pgbuf_inspector=yes in its cubrid.conf "
                         "and restart that database server (startup-only parameter).")
    return f"{database}: enable_pgbuf_inspector=y in the running server; socket must also pass"


def check_producer(path):
    """Probe transport and greeting only; Volmap owns full attachment validation."""
    if not path:
        raise ValueError("supply a producer socket path or set VOLMAP_RUNTIME_SOCKET")
    path = Path(os.path.abspath(path))
    try:
        parent, endpoint = path.parent.lstat(), path.lstat()
    except FileNotFoundError:
        raise ValueError("socket or parent directory missing; this does not prove the parameter is off") from None
    uid = os.geteuid()
    if (not stat.S_ISDIR(parent.st_mode) or parent.st_uid != uid
            or stat.S_IMODE(parent.st_mode) != 0o700):
        raise ValueError("socket parent must be a non-symlink directory owned by this UID with mode 0700")
    if (not stat.S_ISSOCK(endpoint.st_mode) or endpoint.st_uid != uid
            or stat.S_IMODE(endpoint.st_mode) != 0o600):
        raise ValueError("socket must be owned by this UID with mode 0600 (no symlink)")
    deadline = time.monotonic() + TIMEOUT
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
        connection.settimeout(TIMEOUT)
        connection.connect(str(path))
        if not hasattr(socket, "SO_PEERCRED"):
            raise ValueError("peer verification requires Linux SO_PEERCRED")
        _, peer_uid, _ = struct.unpack("3i", connection.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
        current = path.lstat()
        if (peer_uid != uid or (current.st_dev, current.st_ino, current.st_uid, current.st_mode)
                != (endpoint.st_dev, endpoint.st_ino, endpoint.st_uid, endpoint.st_mode)):
            raise ValueError("producer UID mismatch or socket changed during connection")
        connection.sendall(b'{"type":"client_hello","supported_majors":[1]}\n')
        raw = bytearray()
        while b"\n" not in raw:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("producer greeting timed out")
            connection.settimeout(remaining)
            chunk = connection.recv(min(4096, MAX_JSON + 1 - len(raw)))
            if not chunk:
                raise ValueError("producer closed before a complete greeting")
            raw.extend(chunk)
            if len(raw) > MAX_JSON:
                raise ValueError("producer greeting exceeds 64 KiB")
        frame, _, trailing = raw.partition(b"\n")
        if trailing:
            raise ValueError("unexpected data after producer greeting")
        hello = decode(frame)
    if hello.get("type") == "error":
        raise ValueError(f"producer refused greeting: {hello.get('code', 'unknown')}")
    if (hello.get("type") != "server_hello" or type(hello.get("protocol_major")) is not int
            or hello["protocol_major"] != 1):
        raise ValueError("expected a protocol-v1 server greeting")
    for key in ("protocol_minor", "shared_lru_count", "private_lru_count"):
        if type(hello.get(key)) is not int or not 0 <= hello[key] <= 2147483647:
            raise ValueError(f"invalid greeting field: {key}")
    if not re.fullmatch(r"[0-9a-f]{32}", str(hello.get("incarnation", ""))):
        raise ValueError("invalid producer incarnation")
    if not isinstance(hello.get("volumes"), list) or not hello["volumes"]:
        raise ValueError("producer supplies no volume identities")
    seen = set()
    for volume in hello["volumes"]:
        volid = volume.get("volid")
        if type(volid) is not int or not 0 <= volid <= 32767 or volid in seen:
            raise ValueError("invalid or duplicate volume identity")
        seen.add(volid)
        for key in ("volume_creation", "device", "inode"):
            decimal(volume.get(key))
    decimal(hello.get("database_creation"))
    return (f"reachable; same-UID protocol 1.{hello['protocol_minor']} greeting accepted. "
            "Database/volume matching and scan availability still require Volmap attachment.")


def decimal(value):
    if (not isinstance(value, str) or not re.fullmatch(r"0|[1-9][0-9]{0,19}", value)
            or int(value) > 18446744073709551615):
        raise ValueError("invalid producer identity value")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--database", default="demodb", help="running database queried with paramdump -C")
    parser.add_argument("--runtime-socket", default="", help="defaults to VOLMAP_RUNTIME_SOCKET")
    parser.add_argument("--preview-url", default="http://192.168.4.2:7777")
    args = parser.parse_args()
    passed = True
    for label, check, value in (
        ("Synthetic preview", check_preview, args.preview_url),
        ("CUBRID parameter", check_parameter, args.database),
        ("CUBRID producer", check_producer, args.runtime_socket or os.environ.get("VOLMAP_RUNTIME_SOCKET", "")),
    ):
        try:
            print(f"[OK] {label}: {check(value)}")
        except (OSError, ValueError, TypeError, AttributeError, RecursionError,
                http.client.HTTPException, subprocess.TimeoutExpired) as error:
            print(f"[FAIL] {label}: {error}")
            passed = False
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
