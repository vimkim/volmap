#![no_main]

mod common;

use libfuzzer_sys::fuzz_target;
use volmap::format::{
    PageType, decode_bigone_target, decode_btree_page, decode_catalog_class_info,
    decode_catalog_directory, decode_catalog_page, decode_catalog_representation_header,
    decode_heap_page, decode_heap_record_envelope, decode_oos_chunk, decode_page_envelope,
    decode_relocation_target, decode_slotted_page,
};

fuzz_target!(|data: &[u8]| {
    let page = common::normalized_page(
        data,
        &[
            PageType::Heap,
            PageType::Oos,
            PageType::Btree,
            PageType::Catalog,
            PageType::ExtensibleHash,
        ],
    );
    let Some(expected) = common::page_vpid(&page) else {
        return;
    };
    let Ok(envelope) = decode_page_envelope(&page, expected) else {
        return;
    };
    let Ok(slotted) = decode_slotted_page(&envelope) else {
        return;
    };
    match envelope.page_type() {
        PageType::Heap => {
            let _ = decode_heap_page(&envelope, &slotted, true);
            let _ = decode_heap_page(&envelope, &slotted, false);
            for slot in slotted.slots() {
                let id = slot.slot_id();
                let _ = decode_heap_record_envelope(&envelope, &slotted, id, false);
                let _ = decode_heap_record_envelope(&envelope, &slotted, id, true);
                let _ = decode_relocation_target(&envelope, &slotted, id);
                let _ = decode_bigone_target(&envelope, &slotted, id);
            }
        }
        PageType::Oos => {
            for slot in slotted.slots() {
                let _ = decode_oos_chunk(&envelope, &slotted, slot.slot_id());
            }
        }
        PageType::Btree => {
            let _ = decode_btree_page(&envelope, &slotted, false);
            let _ = decode_btree_page(&envelope, &slotted, true);
        }
        PageType::Catalog => {
            let _ = decode_catalog_page(&envelope, &slotted);
            for slot in slotted.slots() {
                let id = slot.slot_id();
                let _ = decode_catalog_class_info(&envelope, &slotted, id);
                let _ = decode_catalog_directory(&envelope, &slotted, id);
                let _ = decode_catalog_representation_header(&envelope, &slotted, id);
            }
        }
        PageType::ExtensibleHash => {}
        _ => unreachable!(),
    }
});
