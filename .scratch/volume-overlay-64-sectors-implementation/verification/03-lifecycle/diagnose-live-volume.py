#!/usr/bin/env python3
"""Read-only diagnostic, not a CI gate: fail if a fresh scope has partial omissions."""
import collections
import json
import sys
import urllib.request

base = sys.argv[1] if len(sys.argv) > 1 else "http://192.168.4.2:7777"
with urllib.request.urlopen(base + "/api/v1/session", timeout=5) as response:
    generation = json.load(response)["snapshot"]["generation"]
payload = {"scope": {"kind": "volume", "volid": 1, "sectorids": list(range(64))},
           "generation": generation, "epoch": "987654321", "cadence_ms": 2000,
           "after_request": True, "retry": False}
request = urllib.request.Request(base + "/api/v1/runtime/page-buffer/observe",
    data=json.dumps(payload).encode(), headers={"Content-Type": "application/json", "Origin": base})
with urllib.request.urlopen(request, timeout=5) as response:
    data = json.load(response)
reasons = collections.Counter(row["reason"] for row in data["slots"] if row)
capture = data.get("capture") or {}
result = {"requested": data["requested_count"], "evaluated": data["evaluated_count"],
          "producer_complete": data["producer_complete"], "capture": capture, "reasons": dict(reasons)}
print(json.dumps(result, indent=2))
if reasons["partial-omission"]:
    print("FAIL: producer partial omissions leave requested pages unknown", file=sys.stderr)
    sys.exit(1)
