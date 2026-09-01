use serde_json::json;
use volmap::projection::{
    BytesWithheldProjection, ClassNameProjection, FileAssociationBodyProjection,
    FileAssociationProjection, OidProjection, OptionalCountProjection, OptionalOidProjection,
    OptionalTextProjection, PageOccupancyProjection, PageProjection, SCHEMA_VERSION,
};

fn page(file_association: FileAssociationProjection) -> PageProjection {
    PageProjection {
        vol_id: 1,
        page_id: 1000,
        sector_id: 15,
        allocation: "allocated",
        page_type: OptionalTextProjection::Known("heap"),
        availability: "available",
        tde_state: "not-encrypted",
        detail_support: OptionalTextProjection::Known("semantic"),
        occupancy: PageOccupancyProjection::Known {
            occupied_percent: 25,
            free_percent: 75,
        },
        lsa_word: OptionalCountProjection::Known("42".to_owned()),
        diagnostic: OptionalTextProjection::Unknown,
        bytes: BytesWithheldProjection {
            state: "bytes-withheld",
        },
        file_association,
    }
}

fn resolved_file() -> FileAssociationBodyProjection {
    FileAssociationBodyProjection {
        vol_id: 1,
        file_id: 640,
        file_type: OptionalTextProjection::Known("heap-reuse-slots"),
        class_oid: OptionalOidProjection::Present {
            oid: OidProjection {
                vol_id: 0,
                page_id: 209,
                slot_id: 2,
            },
        },
        class_name: ClassNameProjection::Resolved {
            value: "dba.poc_table".to_owned(),
        },
    }
}

#[test]
fn schema_one_page_association_is_one_deterministic_additive_field() {
    let page = page(FileAssociationProjection::Allocated {
        file: resolved_file(),
    });
    let first = serde_json::to_string(&page).unwrap();
    let second = serde_json::to_string(&page).unwrap();

    assert_eq!(SCHEMA_VERSION, 1);
    assert_eq!(first, second);
    assert_eq!(
        first,
        r#"{"vol_id":1,"page_id":1000,"sector_id":15,"allocation":"allocated","page_type":{"state":"known","value":"heap"},"availability":"available","tde_state":"not-encrypted","detail_support":{"state":"known","value":"semantic"},"occupancy":{"state":"known","occupied_percent":25,"free_percent":75},"lsa_word":{"state":"known","value":"42"},"diagnostic":{"state":"unknown"},"bytes":{"state":"bytes-withheld"},"file_association":{"state":"allocated","file":{"vol_id":1,"file_id":640,"file_type":{"state":"known","value":"heap-reuse-slots"},"class_oid":{"state":"present","oid":{"vol_id":0,"page_id":209,"slot_id":2}},"class_name":{"state":"resolved","value":"dba.poc_table"}}}}"#
    );

    let mut additive: serde_json::Value = serde_json::from_str(&first).unwrap();
    let association = additive
        .as_object_mut()
        .unwrap()
        .remove("file_association")
        .unwrap();
    assert_eq!(
        association,
        json!({
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
        })
    );
    assert_eq!(
        additive,
        json!({
            "vol_id": 1,
            "page_id": 1000,
            "sector_id": 15,
            "allocation": "allocated",
            "page_type": { "state": "known", "value": "heap" },
            "availability": "available",
            "tde_state": "not-encrypted",
            "detail_support": { "state": "known", "value": "semantic" },
            "occupancy": { "state": "known", "occupied_percent": 25, "free_percent": 75 },
            "lsa_word": { "state": "known", "value": "42" },
            "diagnostic": { "state": "unknown" },
            "bytes": { "state": "bytes-withheld" }
        })
    );
}

#[test]
fn tagged_association_states_omit_inapplicable_members() {
    assert_eq!(
        serde_json::to_value(FileAssociationProjection::None).unwrap(),
        json!({ "state": "none" })
    );
    assert_eq!(
        serde_json::to_value(FileAssociationProjection::MixedClaims).unwrap(),
        json!({ "state": "mixed-claims" })
    );
    assert_eq!(
        serde_json::to_value(ClassNameProjection::Unresolved {
            reason_code: "class-name.page-unavailable",
            reason: "class record page could not be read",
        })
        .unwrap(),
        json!({
            "state": "unresolved",
            "reason_code": "class-name.page-unavailable",
            "reason": "class record page could not be read"
        })
    );
    assert_eq!(
        serde_json::to_value(ClassNameProjection::NotApplicable {
            reason_code: "class-association.oos-deferred",
            reason: "OOS class attribution is intentionally deferred",
        })
        .unwrap(),
        json!({
            "state": "not-applicable",
            "reason_code": "class-association.oos-deferred",
            "reason": "OOS class attribution is intentionally deferred"
        })
    );
}
