//! Throwaway: interpret named rows straight out of a database's volume files.
//!
//! Exercises the format-layer decoders end to end against a real database,
//! resolving each record's class the way the recipe prescribes: slot 0 of the
//! page gives the class OID, the class object's own heap record gives the
//! representation, and `REC_RELOCATION` is followed on the way there.

use std::collections::BTreeMap;
use std::fs;

use volmap::format::{
    AttributeInterpretation, AttributeValue, ClassRepresentationFact, DecodedPageEnvelope,
    HeapPageFact, IO_PAGE_SIZE, PageType, RecordType, RepresentationTarget,
    decode_class_representation, decode_heap_page, decode_heap_record_body, decode_page_envelope,
    decode_relocation_target, decode_slotted_page,
};
use volmap::model::{Oid, PageId, VolId, Vpid};

struct Volumes {
    files: BTreeMap<i16, Vec<u8>>,
}

impl Volumes {
    fn open(paths: &[(i16, &str)]) -> Self {
        let mut files = BTreeMap::new();
        for (id, path) in paths {
            files.insert(*id, fs::read(path).expect("read volume"));
        }
        Self { files }
    }

    fn page(&self, vol: i16, page: i32) -> Option<&[u8; IO_PAGE_SIZE]> {
        let bytes = self.files.get(&vol)?;
        let start = usize::try_from(page).ok()?.checked_mul(IO_PAGE_SIZE)?;
        bytes.get(start..start + IO_PAGE_SIZE)?.try_into().ok()
    }

    fn envelope(&self, vol: i16, page: i32) -> Option<DecodedPageEnvelope<'_>> {
        let raw = self.page(vol, page)?;
        let vpid = Vpid::new(VolId::new(vol).ok()?, PageId::new(page).ok()?);
        decode_page_envelope(raw, vpid).ok()
    }

    fn page_count(&self, vol: i16) -> i32 {
        i32::try_from(self.files[&vol].len() / IO_PAGE_SIZE).unwrap_or(0)
    }

    /// The class OID recorded in slot 0 of a heap page.
    fn page_class(&self, vol: i16, page: i32) -> Option<Oid> {
        let envelope = self.envelope(vol, page)?;
        if envelope.page_type() != PageType::Heap {
            return None;
        }
        let slotted = decode_slotted_page(&envelope).ok()?;
        let fact = decode_heap_page(&envelope, &slotted, true)
            .or_else(|_| decode_heap_page(&envelope, &slotted, false))
            .ok()?;
        match fact {
            HeapPageFact::Header(header) => header.class_oid,
            HeapPageFact::Chain(chain) => chain.class_oid,
        }
    }

    /// Parses the representation `target` from the class object at `oid`,
    /// following a relocation if the class record has been moved.
    fn representation(
        &self,
        oid: Oid,
        target: RepresentationTarget,
    ) -> Option<ClassRepresentationFact> {
        let mut oid = oid;
        for _ in 0..4 {
            let envelope = self.envelope(oid.vol_id.get(), oid.page_id.get())?;
            let slotted = decode_slotted_page(&envelope).ok()?;
            let slot_id = u16::try_from(oid.slot_id.get()).ok()?;
            let slot = slotted.slots().get(usize::from(slot_id))?;
            if slot.record_type() == RecordType::Relocation {
                oid = decode_relocation_target(&envelope, &slotted, slot_id).ok()?;
                continue;
            }
            let (header, body) =
                decode_heap_record_body(&envelope, &slotted, slot_id, true).ok()?;
            return decode_class_representation(
                body,
                header.variable_offset_width,
                header.representation_id,
                target,
            )
            .ok();
        }
        None
    }

    fn render_row(
        &self,
        vol: i16,
        page: i32,
        slot_id: u16,
        rep: &ClassRepresentationFact,
    ) -> String {
        let Some(envelope) = self.envelope(vol, page) else {
            return "<no envelope>".to_owned();
        };
        let Ok(slotted) = decode_slotted_page(&envelope) else {
            return "<not slotted>".to_owned();
        };
        let Ok((header, body)) = decode_heap_record_body(&envelope, &slotted, slot_id, true) else {
            return "<no body>".to_owned();
        };
        // A row written under an older representation resolves against that one.
        let owned;
        let rep = if header.representation_id == rep.representation_id {
            rep
        } else {
            let Some(older) = self.page_class(vol, page).and_then(|oid| {
                self.representation(oid, RepresentationTarget::Id(header.representation_id))
            }) else {
                return format!("<reprid {} unresolved>", header.representation_id);
            };
            owned = older;
            &owned
        };
        match volmap::format::decode_record_interpretation(body, &header, rep) {
            Ok(record) => record
                .attributes
                .iter()
                .map(|attribute| {
                    format!(
                        "{}={}",
                        attribute.name.as_deref().unwrap_or("?"),
                        show(&attribute.interpretation)
                    )
                })
                .collect::<Vec<_>>()
                .join(", "),
            Err(error) => format!("<{error}>"),
        }
    }
}

fn show(interpretation: &AttributeInterpretation) -> String {
    match interpretation {
        AttributeInterpretation::Null => "NULL".to_owned(),
        AttributeInterpretation::OutOfRow { head, total_length } => format!(
            "<out-of-row {}|{}|{} {total_length} bytes>",
            head.vol_id.get(),
            head.page_id.get(),
            head.slot_id.get()
        ),
        AttributeInterpretation::Undecodable { reason, length, .. } => {
            format!("<withheld {length} bytes: {reason}>")
        }
        AttributeInterpretation::Decoded(value) => match value {
            AttributeValue::Integer(v) => v.to_string(),
            AttributeValue::Short(v) => v.to_string(),
            AttributeValue::BigInt(v) => v.to_string(),
            AttributeValue::Float(v) => v.to_string(),
            AttributeValue::Double(v) => v.to_string(),
            AttributeValue::Numeric(v) => v.clone(),
            AttributeValue::Monetary {
                currency_code,
                amount,
            } => {
                format!("money({currency_code}) {amount}")
            }
            AttributeValue::Date(d) => format!("{:04}-{:02}-{:02}", d.year, d.month, d.day),
            AttributeValue::Time(t) => format!("{:02}:{:02}:{:02}", t.hour, t.minute, t.second),
            AttributeValue::Timestamp(v) => format!("epoch {v}"),
            AttributeValue::DateTime {
                date,
                time,
                millisecond,
            } => format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{millisecond:03}",
                date.year, date.month, date.day, time.hour, time.minute, time.second
            ),
            AttributeValue::Text(text) => {
                if text.len() > 60 {
                    format!("'{}…' ({} bytes)", &text[..60], text.len())
                } else {
                    format!("'{text}'")
                }
            }
            AttributeValue::Object(oid) => format!(
                "OID {}|{}|{}",
                oid.vol_id.get(),
                oid.page_id.get(),
                oid.slot_id.get()
            ),
        },
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let db = &args[0];
    let wanted: Vec<&str> = args[1..].iter().map(String::as_str).collect();
    let volumes = Volumes::open(&[(0, db), (1, &format!("{db}_x001"))]);

    // Group heap pages by class, then name each class from its class record.
    let mut by_class: BTreeMap<String, (Oid, Vec<(i16, i32)>)> = BTreeMap::new();
    for vol in [0_i16, 1] {
        for page in 0..volumes.page_count(vol) {
            let Some(oid) = volumes.page_class(vol, page) else {
                continue;
            };
            let Some(rep) = volumes.representation(oid, RepresentationTarget::Current) else {
                continue;
            };
            by_class
                .entry(rep.class_name)
                .or_insert_with(|| (oid, Vec::new()))
                .1
                .push((vol, page));
        }
    }

    for name in wanted {
        let Some((class_name, (oid, pages))) = by_class
            .iter()
            .find(|(class_name, _)| class_name.ends_with(name))
        else {
            println!("== {name}: not found ==");
            continue;
        };
        let rep = volumes
            .representation(*oid, RepresentationTarget::Current)
            .expect("representation");
        println!(
            "== {class_name} (class {}|{}|{}, reprid {}, {} pages) ==",
            oid.vol_id.get(),
            oid.page_id.get(),
            oid.slot_id.get(),
            rep.representation_id,
            pages.len()
        );
        let mut shown = 0;
        for (vol, page) in pages {
            let Some(envelope) = volumes.envelope(*vol, *page) else {
                continue;
            };
            let Ok(slotted) = decode_slotted_page(&envelope) else {
                continue;
            };
            for slot in slotted.slots().iter().skip(1) {
                if !matches!(slot.record_type(), RecordType::Home | RecordType::NewHome)
                    || slot.is_empty()
                {
                    continue;
                }
                println!(
                    "  {vol}|{page}|{}: {}",
                    slot.slot_id(),
                    volumes.render_row(*vol, *page, slot.slot_id(), &rep)
                );
                shown += 1;
                if shown >= 3 {
                    break;
                }
            }
            if shown >= 3 {
                break;
            }
        }
    }
}
