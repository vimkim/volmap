"""Proposed slim wire JSON sizes, not a runtime benchmark or finalized schema."""
import json


def size(value):
    return len(json.dumps(value, separators=(",", ":")).encode())


sector_ids = list(range(33_554_431 - 63, 33_554_431 + 1))
scope = {"kind": "volume", "volid": 32767, "sector_ids": sector_ids}
request = {
    "scope": scope,
    "epoch": "18446744073709551615",
    "generation": "18446744073709551615",
    "retry": False,
    "cadence_ms": 2000,
    "after_request": False,
}
resident = {
    "state": "resident",
    "reason": "observed-resident",
    "lru_zone": "unknown",
    "lru_list_kind": "private",
    "lru_list_index": 4294967295,
}
unknown = {"state": "unknown", "reason": "partial-omission"}
nonresident = {"state": "not-resident", "reason": "observed-not-resident"}
print(f"request_bytes={size(request)}")
for label, row in [("resident_wide", resident), ("partial_unknown", unknown), ("not_resident", nonresident)]:
    body = {
        "scope": scope,
        "sectors": [{"sector_id": sector, "pages": [row] * 64} for sector in sector_ids],
    }
    print(f"{label}_scope_and_sectors_bytes={size(body)}")
print("Common capture/capability/coverage metadata excluded; page identity derives from sector and slot.")
print("Wide synthetic field widths are a conservative sizing model, not a valid producer fixture.")
