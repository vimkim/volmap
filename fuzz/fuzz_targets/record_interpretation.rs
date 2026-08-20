#![no_main]

mod common;

use libfuzzer_sys::fuzz_target;
use volmap::format::{
    ClassRepresentationFact, PageType, RepresentationTarget, decode_class_representation,
    decode_heap_record_body, decode_page_envelope, decode_record_attributes, decode_slotted_page,
};

/// Class-representation parsing and attribute-value decoding over hostile
/// bytes. Neither may panic, allocate unboundedly, or loop forever on any
/// input; every rejection has to arrive as a `DecodeError`.
fuzz_target!(|data: &[u8]| {
    // The direct byte path: arbitrary bytes as a class-record body, with the
    // header facts the parser would otherwise receive taken from the input.
    let (control, body) = data.split_at(data.len().min(6));
    let width = control.first().copied().unwrap_or(4);
    let representation_id = u32::from(control.get(1).copied().unwrap_or_default());
    let targets = [
        RepresentationTarget::Current,
        RepresentationTarget::Id(representation_id),
        RepresentationTarget::Id(representation_id.wrapping_add(1)),
    ];
    for target in targets {
        if let Ok(representation) =
            decode_class_representation(body, width, representation_id, target)
        {
            interpret_with(body, width, &representation);
        }
    }

    // The page path, so realistic record and slot geometry is exercised too.
    let page = common::normalized_page(data, &[PageType::Heap]);
    let Some(vpid) = common::page_vpid(&page) else {
        return;
    };
    let Ok(envelope) = decode_page_envelope(&page, vpid) else {
        return;
    };
    let Ok(slotted) = decode_slotted_page(&envelope) else {
        return;
    };
    for slot in slotted.slots() {
        for is_mvcc in [false, true] {
            let Ok((header, body)) =
                decode_heap_record_body(&envelope, &slotted, slot.slot_id(), is_mvcc)
            else {
                continue;
            };
            for target in [
                RepresentationTarget::Current,
                RepresentationTarget::Id(header.representation_id),
                RepresentationTarget::Id(header.representation_id.wrapping_add(1)),
            ] {
                if let Ok(representation) = decode_class_representation(
                    body,
                    header.variable_offset_width,
                    header.representation_id,
                    target,
                ) {
                    interpret_with(body, header.variable_offset_width, &representation);
                }
            }
        }
    }
});

/// Interprets a body against a representation that may describe entirely
/// different bytes, which is the case a corrupt or misresolved class record
/// produces in the field.
fn interpret_with(body: &[u8], width: u8, representation: &ClassRepresentationFact) {
    for has_bound_bits in [false, true] {
        let _ = decode_record_attributes(body, width, has_bound_bits, representation);
    }
}
