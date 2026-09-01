# Page file/class association contract

Schema version 1 Page projections contain one additive `file_association` field. It is the shared, presentation-neutral result of joining validated inspection-graph facts:

```text
Page (VPID) --allocated by / reserved for--> File (VFID)
File (VFID) -> descriptor class OID -> stored class name
```

Adapters consume this projection. They do not reopen volume bytes, derive ownership from physical Page type, or search payloads for names.

## JSON shape

`file_association` is always present and is tagged by `state`:

| State | Meaning | Additional member |
|---|---|---|
| `none` | No validated allocation or reservation relationship | none |
| `mixed-claims` | Competing reservation claims prevent selecting one File | none |
| `allocated` | Exactly one validated File allocates the Page | `file` |
| `reserved-for` | The Page is unallocated inside exactly one File's reserved sector | `file` |

The `file` object contains numeric `vol_id` and `file_id`, a tagged `file_type`, a tagged `class_oid`, and a tagged `class_name`. A known class OID remains present when only name resolution fails.

```json
{
  "file_association": {
    "state": "allocated",
    "file": {
      "vol_id": 1,
      "file_id": 640,
      "file_type": { "state": "known", "value": "heap-reuse-slots" },
      "class_oid": {
        "state": "present",
        "oid": { "vol_id": 0, "page_id": 209, "slot_id": 2 }
      },
      "class_name": { "state": "resolved", "value": "dba.poc_table" }
    }
  }
}
```

An unresolved result retains the exact descriptor OID and carries a stable
machine reason:

```json
{
  "file_association": {
    "state": "allocated",
    "file": {
      "vol_id": 0,
      "file_id": 320,
      "file_type": { "state": "known", "value": "extensible-hash" },
      "class_oid": {
        "state": "present",
        "oid": { "vol_id": 0, "page_id": 193, "slot_id": 1 }
      },
      "class_name": {
        "state": "unresolved",
        "reason_code": "class.name.var_table_order",
        "reason": "class record format validation failed"
      }
    }
  }
}
```

An internal File does not acquire a table name merely because it allocates a
Page:

```json
{
  "file_association": {
    "state": "allocated",
    "file": {
      "vol_id": 0,
      "file_id": 576,
      "file_type": { "state": "known", "value": "catalog" },
      "class_oid": { "state": "absent" },
      "class_name": {
        "state": "not-applicable",
        "reason_code": "class-association.internal-file",
        "reason": "internal file is not associated with one user class"
      }
    }
  }
}
```

`class_name` has three states:

- `resolved` carries only `value`.
- `unresolved` carries stable `reason_code` and explanatory `reason`.
- `not-applicable` carries stable `reason_code` and explanatory `reason`.

Automation uses `state` and `reason_code`; `reason` is display text. The following association and common name-resolution codes are stable schema-version-1 values:

| Reason code | Meaning |
|---|---|
| `class-association.inventory-incomplete` | A complete validated File inventory is required |
| `class-association.null-oid` | The descriptor contains CUBRID's null class OID |
| `class-association.no-single-class` | The File family has no single class association |
| `class-association.internal-file` | The File is internal rather than user-class-owned |
| `class-association.oos-deferred` | OOS attribution is outside the supported profile |
| `class-name.not-resolved` | No class-name resolution was published |
| `class-name.codeset-inventory-incomplete` | Database codeset evidence is incomplete |
| `class-name.codeset-inconsistent` | Volume headers disagree on the codeset |
| `class-name.codeset-unsupported` | The stored database codeset is unsupported |
| `class-name.page-unavailable` | The exact class-record Page could not be read |
| `class-name.page-encrypted-opaque` | The class-record Page cannot be decrypted without a key |
| `class-name.page-decryption-failed` | Decryption failed |
| `class-name.slot-invalid` | The descriptor's Slot identifier is invalid |
| `class-name.slot-missing` | The exact Slot is absent |
| `class-name.record-not-live` | The exact Slot is not a live class record |
| `class-name.relocation-cycle` | A validated relocation walk encountered a cycle |
| `class-name.relocation-limit` | The relocation bound stopped the walk |
| `class-name.interrupted` | The operation was cancelled |
| `class-name.resource-limit` | The operation reached its resource policy |
| `class-name.identifier-invalid` | Identifier length or contents are invalid |
| `class-name.identifier-non-ascii` | An ASCII database stores non-ASCII name bytes |
| `class-name.identifier-ascii-invalid` | ASCII decoding failed |
| `class-name.identifier-euc-kr-invalid` | EUC-KR decoding failed |
| `class-name.identifier-utf8-invalid` | UTF-8 decoding failed |

Named validation-rule identifiers such as `class.name.*`, `heap.relocation.*`, `overflow.chain.*`, and `slotted.*` may also appear as `reason_code` when that exact boundary rejects the class record. Released meanings are not repurposed.

## Presentation and evidence

JSON and JSONL carry the Page projection directly. The TUI, live web, and
frozen HTML Page panels consume it and label its members `File`, `File role`,
`Class OID`, and `Class/table`; they do not derive a second association. The
finite `human` renderer does not currently print these rows. Display text may
be formatted for an interactive adapter, but automation relies on the tagged
JSON states and reason codes above.

Name resolution requires a stable offline snapshot with a complete validated
volume and File inventory. It uses the database codeset retained from
consistent volume headers. Supported codesets are ASCII, ISO-8859-1, EUC-KR,
and UTF-8. Missing headers, inconsistent or unsupported codesets, unreadable or
encrypted Pages without usable keys, malformed records, cancellation, and
resource bounds remain typed unresolved evidence. Resolution never connects to
a running server, invokes CUBRID utilities, depends on a CUBRID source tree, or
searches arbitrary bytes for a plausible name.

## Compatibility and limitations

The association is an additive schema-version-1 field: every prior Page field and meaning remains unchanged, and consumers following the schema-version-1 rule ignore unknown additive object fields. Variant-specific members are omitted rather than encoded as `null`. Adding this field therefore does not increment `schema_version`; removing it or changing an existing state or field meaning would require a new schema version.

JSON and JSONL use ordinary UTF-8 JSON escaping. The deterministic HTML export additionally escapes HTML-active characters in its embedded JSON; parsing that JSON restores the exact stored class name.

Attribution is fail-closed and snapshot-scoped. Incomplete or conflicting inventory publishes no inferred owner. A reserved-but-unallocated Page is related as `reserved-for`, never `allocated`. Catalog/global/internal Files are not user-table-owned. OOS class attribution remains explicitly deferred. Class-name resolution reads only the descriptor's exact OID through bounded validated Page, Slot, relocation, and multipage-overflow operations; it never scans arbitrary strings or consults a running database.

This is a Page contract. A sector-level attribution displayed elsewhere is a
separate summary of validated File claims and must preserve unclaimed, mixed,
and partial-claim distinctions; it is not evidence that every Page in the
sector belongs to one table. OOS storage-chain inspection remains available as
its own structural feature, but `FILE_OOS` does not gain a table name through
this contract.
