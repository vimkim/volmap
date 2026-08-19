//! Pinned special-heap records needed to bootstrap offline TDE inspection.

use crate::model::{FileId, Hfid, PageId, Vfid, VolId};

use super::{DecodeError, DecodeErrorKind, DecodedPageEnvelope, PageType, RecordType, SlottedPage};

pub const BOOT_DB_PARM_SIZE: usize = 136;
pub const TDE_KEY_INFO_RECORD_SIZE: usize = 156;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootDbParmFact {
    pub tde_keyinfo_hfid: Hfid,
}

/// Copy the unique live `REC_HOME` payload from a caller-proven special heap
/// page. Deleted address slots are ignored; forwarding and ambiguous live
/// records fail closed because these two bootstrap records are always small.
pub(crate) fn copy_special_heap_record<const N: usize>(
    envelope: &DecodedPageEnvelope<'_>,
    slotted: &SlottedPage,
) -> Result<Option<[u8; N]>, DecodeError> {
    if envelope.page_type() != PageType::Heap {
        return Err(error(
            DecodeErrorKind::WrongPageType,
            "boot.special_heap.page_type",
        ));
    }
    let view = envelope.plaintext("boot.special_heap.encrypted")?;
    let mut record = None;
    for slot in slotted.slots().iter().skip(1) {
        match slot.record_type() {
            RecordType::AssignAddress | RecordType::MarkDeleted | RecordType::DeletedWillReuse => {}
            RecordType::Home if slot.offset() != 0 && usize::from(slot.length()) == N => {
                if record.is_some() {
                    return Err(error(
                        DecodeErrorKind::InvalidGeometry,
                        "boot.special_heap.record_unique",
                    ));
                }
                let bytes = view
                    .range(usize::from(slot.offset()), N, "special heap record")
                    .map_err(|_| {
                        error(
                            DecodeErrorKind::ByteAccess,
                            "boot.special_heap.record_bounds",
                        )
                    })?;
                record = Some(bytes.try_into().map_err(|_| {
                    error(
                        DecodeErrorKind::InvalidLength,
                        "boot.special_heap.record_size",
                    )
                })?);
            }
            RecordType::Home
            | RecordType::NewHome
            | RecordType::Relocation
            | RecordType::BigOne
            | RecordType::Unknown
            | RecordType::Reserved(_) => {
                return Err(error(
                    DecodeErrorKind::InvalidGeometry,
                    "boot.special_heap.record_shape",
                ));
            }
        }
    }
    Ok(record)
}

/// Decode only the boot fields required to locate `TDE_KEYINFO`, while also
/// proving that the record identifies the same boot heap as volume zero.
pub fn decode_boot_db_parm(
    record: &[u8; BOOT_DB_PARM_SIZE],
    expected_boot: Hfid,
) -> Result<BootDbParmFact, DecodeError> {
    let own_hfid = decode_hfid(record, 8, "boot.db_parm.self_hfid")?;
    if own_hfid != expected_boot {
        return Err(error(
            DecodeErrorKind::IdentityMismatch,
            "boot.db_parm.self_hfid",
        ));
    }
    Ok(BootDbParmFact {
        tde_keyinfo_hfid: decode_hfid(record, 124, "boot.db_parm.tde_keyinfo_hfid")?,
    })
}

fn decode_hfid(record: &[u8], offset: usize, rule: &'static str) -> Result<Hfid, DecodeError> {
    let file_id = i32::from_le_bytes(read_array(record, offset, rule)?);
    let vol_id = i16::from_le_bytes(read_array(record, offset + 4, rule)?);
    let header_page_id = i32::from_le_bytes(read_array(record, offset + 8, rule)?);
    Ok(Hfid::new(
        Vfid::new(
            VolId::new(vol_id).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
            FileId::new(file_id).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        ),
        PageId::new(header_page_id).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
    ))
}

fn read_array<const N: usize>(
    bytes: &[u8],
    offset: usize,
    rule: &'static str,
) -> Result<[u8; N], DecodeError> {
    let end = offset
        .checked_add(N)
        .ok_or_else(|| error(DecodeErrorKind::ArithmeticOverflow, rule))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| error(DecodeErrorKind::ByteAccess, rule))?
        .try_into()
        .map_err(|_| error(DecodeErrorKind::InvalidLength, rule))
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
