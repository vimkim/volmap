//! Checked, read-only access to an already bounded byte container.

use core::fmt;

/// The class of failure produced before any read or slice is attempted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ByteAccessErrorKind {
    ArithmeticOverflow,
    OutOfBounds,
    NegativeValue,
    ConversionOverflow,
    InvalidAlignment,
}

/// Safe location metadata for a failed byte-access operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteAccessError {
    kind: ByteAccessErrorKind,
    field: &'static str,
    container_len: usize,
    relative_offset: usize,
    requested_len: usize,
}

impl ByteAccessError {
    #[must_use]
    pub const fn kind(&self) -> ByteAccessErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }

    #[must_use]
    pub const fn container_len(&self) -> usize {
        self.container_len
    }

    #[must_use]
    pub const fn relative_offset(&self) -> usize {
        self.relative_offset
    }

    #[must_use]
    pub const fn requested_len(&self) -> usize {
        self.requested_len
    }

    const fn arithmetic(field: &'static str) -> Self {
        Self {
            kind: ByteAccessErrorKind::ArithmeticOverflow,
            field,
            container_len: 0,
            relative_offset: 0,
            requested_len: 0,
        }
    }
}

impl fmt::Display for ByteAccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {:?}", self.field, self.kind)
    }
}

impl std::error::Error for ByteAccessError {}

/// A borrowed byte container with a checked absolute origin.
///
/// The interface exposes only checked reads and subviews. It never casts bytes
/// to a native-layout structure and never owns or mutates the input.
#[derive(Clone, Copy)]
pub struct ByteView<'a> {
    bytes: &'a [u8],
    origin: u64,
}

impl fmt::Debug for ByteView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ByteView")
            .field("origin", &self.origin)
            .field("len", &self.bytes.len())
            .finish()
    }
}

impl<'a> ByteView<'a> {
    #[must_use]
    pub const fn new(bytes: &'a [u8], origin: u64) -> Self {
        Self { bytes, origin }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    #[must_use]
    pub const fn origin(&self) -> u64 {
        self.origin
    }

    pub fn read_u16_le(&self, offset: usize, field: &'static str) -> Result<u16, ByteAccessError> {
        Ok(u16::from_le_bytes(self.array(offset, field)?))
    }

    pub fn read_i16_le(&self, offset: usize, field: &'static str) -> Result<i16, ByteAccessError> {
        Ok(i16::from_le_bytes(self.array(offset, field)?))
    }

    pub fn read_u32_le(&self, offset: usize, field: &'static str) -> Result<u32, ByteAccessError> {
        Ok(u32::from_le_bytes(self.array(offset, field)?))
    }

    pub fn read_i32_le(&self, offset: usize, field: &'static str) -> Result<i32, ByteAccessError> {
        Ok(i32::from_le_bytes(self.array(offset, field)?))
    }

    pub fn read_u64_le(&self, offset: usize, field: &'static str) -> Result<u64, ByteAccessError> {
        Ok(u64::from_le_bytes(self.array(offset, field)?))
    }

    pub fn read_i64_le(&self, offset: usize, field: &'static str) -> Result<i64, ByteAccessError> {
        Ok(i64::from_le_bytes(self.array(offset, field)?))
    }

    pub fn read_u8(&self, offset: usize, field: &'static str) -> Result<u8, ByteAccessError> {
        Ok(self.range(offset, 1, field)?[0])
    }

    pub fn subview(
        &self,
        offset: usize,
        length: usize,
        field: &'static str,
    ) -> Result<Self, ByteAccessError> {
        let bytes = self.range(offset, length, field)?;
        let relative = u64::try_from(offset).map_err(|_| ByteAccessError::arithmetic(field))?;
        let origin = self
            .origin
            .checked_add(relative)
            .ok_or_else(|| ByteAccessError::arithmetic(field))?;
        Ok(Self { bytes, origin })
    }

    pub fn range(
        &self,
        offset: usize,
        length: usize,
        field: &'static str,
    ) -> Result<&'a [u8], ByteAccessError> {
        let end = offset.checked_add(length).ok_or(ByteAccessError {
            kind: ByteAccessErrorKind::ArithmeticOverflow,
            field,
            container_len: self.len(),
            relative_offset: offset,
            requested_len: length,
        })?;
        if end > self.len() {
            return Err(ByteAccessError {
                kind: ByteAccessErrorKind::OutOfBounds,
                field,
                container_len: self.len(),
                relative_offset: offset,
                requested_len: length,
            });
        }

        let absolute_offset = u64::try_from(offset)
            .ok()
            .and_then(|relative| self.origin.checked_add(relative));
        let absolute_end = u64::try_from(length)
            .ok()
            .and_then(|requested| absolute_offset?.checked_add(requested));
        if absolute_end.is_none() {
            return Err(ByteAccessError {
                kind: ByteAccessErrorKind::ArithmeticOverflow,
                field,
                container_len: self.len(),
                relative_offset: offset,
                requested_len: length,
            });
        }

        self.bytes.get(offset..end).ok_or(ByteAccessError {
            kind: ByteAccessErrorKind::OutOfBounds,
            field,
            container_len: self.len(),
            relative_offset: offset,
            requested_len: length,
        })
    }

    fn array<const N: usize>(
        &self,
        offset: usize,
        field: &'static str,
    ) -> Result<[u8; N], ByteAccessError> {
        self.range(offset, N, field)?
            .try_into()
            .map_err(|_| ByteAccessError {
                kind: ByteAccessErrorKind::OutOfBounds,
                field,
                container_len: self.len(),
                relative_offset: offset,
                requested_len: N,
            })
    }
}

pub fn non_negative_i16(value: i16, field: &'static str) -> Result<u16, ByteAccessError> {
    u16::try_from(value).map_err(|_| ByteAccessError {
        kind: ByteAccessErrorKind::NegativeValue,
        field,
        container_len: 0,
        relative_offset: 0,
        requested_len: 0,
    })
}

pub fn non_negative_i32(value: i32, field: &'static str) -> Result<u32, ByteAccessError> {
    u32::try_from(value).map_err(|_| ByteAccessError {
        kind: ByteAccessErrorKind::NegativeValue,
        field,
        container_len: 0,
        relative_offset: 0,
        requested_len: 0,
    })
}

pub fn checked_add(left: u64, right: u64, field: &'static str) -> Result<u64, ByteAccessError> {
    left.checked_add(right)
        .ok_or_else(|| ByteAccessError::arithmetic(field))
}

pub fn checked_mul(left: u64, right: u64, field: &'static str) -> Result<u64, ByteAccessError> {
    left.checked_mul(right)
        .ok_or_else(|| ByteAccessError::arithmetic(field))
}

pub fn checked_align_up(
    value: u64,
    alignment: u64,
    field: &'static str,
) -> Result<u64, ByteAccessError> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(ByteAccessError {
            kind: ByteAccessErrorKind::InvalidAlignment,
            field,
            container_len: 0,
            relative_offset: 0,
            requested_len: 0,
        });
    }
    let mask = alignment - 1;
    checked_add(value, mask, field).map(|sum| sum & !mask)
}
