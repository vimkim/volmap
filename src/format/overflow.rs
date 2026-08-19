use crate::bytes::ByteView;
use crate::model::{PageId, VolId, Vpid};

use super::{DB_PAGE_SIZE, DecodeError, DecodeErrorKind, DecodedPageEnvelope, PageType};

const FIRST_HEADER_SIZE: u16 = 12;
const CONTINUATION_HEADER_SIZE: u16 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverflowPageFact {
    next: Option<Vpid>,
    total_length: Option<u32>,
    payload_offset: u16,
    payload_length: u16,
}

impl OverflowPageFact {
    #[must_use]
    pub const fn next(self) -> Option<Vpid> {
        self.next
    }

    #[must_use]
    pub const fn total_length(self) -> Option<u32> {
        self.total_length
    }

    #[must_use]
    pub const fn payload_offset(self) -> u16 {
        self.payload_offset
    }

    #[must_use]
    pub const fn payload_length(self) -> u16 {
        self.payload_length
    }
}

pub fn decode_overflow_head(
    envelope: &DecodedPageEnvelope<'_>,
) -> Result<OverflowPageFact, DecodeError> {
    let view = overflow_view(envelope)?;
    let next = optional_vpid(&view, 0, "overflow.head.next")?;
    let length = view
        .read_i32_le(8, "overflow head length")
        .map_err(|_| error(DecodeErrorKind::ByteAccess, "overflow.head.length"))?;
    let total_length = u32::try_from(length)
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| error(DecodeErrorKind::InvalidLength, "overflow.head.length"))?;
    let capacity = DB_PAGE_SIZE - usize::from(FIRST_HEADER_SIZE);
    let payload_length = usize::try_from(total_length)
        .unwrap_or(usize::MAX)
        .min(capacity);
    validate_link_shape(
        next,
        usize::try_from(total_length).unwrap_or(usize::MAX) > capacity,
    )?;
    Ok(OverflowPageFact {
        next,
        total_length: Some(total_length),
        payload_offset: FIRST_HEADER_SIZE,
        payload_length: u16::try_from(payload_length)
            .map_err(|_| error(DecodeErrorKind::ArithmeticOverflow, "overflow.head.payload"))?,
    })
}

pub fn decode_overflow_continuation(
    envelope: &DecodedPageEnvelope<'_>,
    remaining_length: u32,
) -> Result<OverflowPageFact, DecodeError> {
    if remaining_length == 0 {
        return Err(error(
            DecodeErrorKind::InvalidLength,
            "overflow.continuation.remaining",
        ));
    }
    let view = overflow_view(envelope)?;
    let next = optional_vpid(&view, 0, "overflow.continuation.next")?;
    let capacity = DB_PAGE_SIZE - usize::from(CONTINUATION_HEADER_SIZE);
    let remaining = usize::try_from(remaining_length).unwrap_or(usize::MAX);
    let payload_length = remaining.min(capacity);
    validate_link_shape(next, remaining > capacity)?;
    Ok(OverflowPageFact {
        next,
        total_length: None,
        payload_offset: CONTINUATION_HEADER_SIZE,
        payload_length: u16::try_from(payload_length).map_err(|_| {
            error(
                DecodeErrorKind::ArithmeticOverflow,
                "overflow.continuation.payload",
            )
        })?,
    })
}

fn overflow_view<'a>(envelope: &'a DecodedPageEnvelope<'a>) -> Result<ByteView<'a>, DecodeError> {
    if envelope.page_type() != PageType::Overflow {
        return Err(error(DecodeErrorKind::WrongPageType, "overflow.page.type"));
    }
    envelope.plaintext("overflow.page.encrypted")
}

fn validate_link_shape(next: Option<Vpid>, more_payload: bool) -> Result<(), DecodeError> {
    if next.is_some() != more_payload {
        return Err(error(
            DecodeErrorKind::InvalidGeometry,
            "overflow.page.link_shape",
        ));
    }
    Ok(())
}

fn optional_vpid(
    view: &ByteView<'_>,
    offset: usize,
    rule: &'static str,
) -> Result<Option<Vpid>, DecodeError> {
    let page = view
        .read_i32_le(offset, "overflow next page")
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))?;
    let volume = view
        .read_i16_le(offset + 4, "overflow next volume")
        .map_err(|_| error(DecodeErrorKind::ByteAccess, rule))?;
    if page == -1 && volume == -1 {
        return Ok(None);
    }
    if page < 0 || volume < 0 {
        return Err(error(DecodeErrorKind::InvalidGeometry, rule));
    }
    Ok(Some(Vpid::new(
        VolId::new(volume).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
        PageId::new(page).map_err(|_| error(DecodeErrorKind::OutOfRange, rule))?,
    )))
}

const fn error(kind: DecodeErrorKind, rule: &'static str) -> DecodeError {
    DecodeError::new(kind, rule)
}
