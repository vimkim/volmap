use crate::bytes::ByteView;
use crate::model::Vpid;

use super::{DecodeError, DecodeErrorKind, FormatProfile};

pub const IO_PAGE_SIZE: usize = 16_384;
pub const DB_PAGE_SIZE: usize = 16_344;
pub const PAGE_PREFIX_SIZE: usize = 32;
pub const PAGE_WATERMARK_SIZE: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageType {
    Unknown,
    FileTable,
    Heap,
    VolumeHeader,
    VolumeBitmap,
    QueryResult,
    ExtensibleHash,
    Overflow,
    Oos,
    Area,
    Catalog,
    Btree,
    Log,
    DroppedFiles,
    VacuumData,
}

impl PageType {
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::FileTable => 1,
            Self::Heap => 2,
            Self::VolumeHeader => 3,
            Self::VolumeBitmap => 4,
            Self::QueryResult => 5,
            Self::ExtensibleHash => 6,
            Self::Overflow => 7,
            Self::Oos => 8,
            Self::Area => 9,
            Self::Catalog => 10,
            Self::Btree => 11,
            Self::Log => 12,
            Self::DroppedFiles => 13,
            Self::VacuumData => 14,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::FileTable => "file-table",
            Self::Heap => "heap",
            Self::VolumeHeader => "volume-header",
            Self::VolumeBitmap => "volume-bitmap",
            Self::QueryResult => "query-result",
            Self::ExtensibleHash => "extensible-hash",
            Self::Overflow => "overflow",
            Self::Oos => "oos",
            Self::Area => "area",
            Self::Catalog => "catalog",
            Self::Btree => "btree",
            Self::Log => "log",
            Self::DroppedFiles => "dropped-files",
            Self::VacuumData => "vacuum-data",
        }
    }

    fn from_ordinal(value: u8, profile: FormatProfile) -> Result<Self, DecodeError> {
        match (profile, value) {
            (_, 0) => Ok(Self::Unknown),
            (_, 1) => Ok(Self::FileTable),
            (_, 2) => Ok(Self::Heap),
            (_, 3) => Ok(Self::VolumeHeader),
            (_, 4) => Ok(Self::VolumeBitmap),
            (_, 5) => Ok(Self::QueryResult),
            (_, 6) => Ok(Self::ExtensibleHash),
            (_, 7) => Ok(Self::Overflow),
            (FormatProfile::FeatOos, 8) => Ok(Self::Oos),
            (FormatProfile::Develop, 8) | (FormatProfile::FeatOos, 9) => Ok(Self::Area),
            (FormatProfile::Develop, 9) | (FormatProfile::FeatOos, 10) => Ok(Self::Catalog),
            (FormatProfile::Develop, 10) | (FormatProfile::FeatOos, 11) => Ok(Self::Btree),
            (FormatProfile::Develop, 11) | (FormatProfile::FeatOos, 12) => Ok(Self::Log),
            (FormatProfile::Develop, 12) | (FormatProfile::FeatOos, 13) => Ok(Self::DroppedFiles),
            (FormatProfile::Develop, 13) | (FormatProfile::FeatOos, 14) => Ok(Self::VacuumData),
            _ => Err(DecodeError::new(
                DecodeErrorKind::UnknownEnum,
                "page.envelope.type_known",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TdeAlgorithm {
    Aes,
    Aria,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageContent {
    Plaintext,
    Decrypted { algorithm: TdeAlgorithm },
    EncryptedOpaque { algorithm: TdeAlgorithm },
}

#[derive(Clone, Copy, Debug)]
pub struct DecodedPageEnvelope<'a> {
    id: Vpid,
    profile: FormatProfile,
    page_type: PageType,
    lsa_word: u64,
    content: PageContent,
    plaintext: Option<ByteView<'a>>,
}

/// Owned facts available from the plaintext prefix and trailing watermark.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageEnvelopeSummary {
    id: Vpid,
    profile: FormatProfile,
    page_type: PageType,
    lsa_word: u64,
    content: PageContent,
}

impl PageEnvelopeSummary {
    #[must_use]
    pub const fn profile(self) -> FormatProfile {
        self.profile
    }

    #[must_use]
    pub const fn id(self) -> Vpid {
        self.id
    }

    #[must_use]
    pub const fn page_type(self) -> PageType {
        self.page_type
    }

    #[must_use]
    pub const fn lsa_word(self) -> u64 {
        self.lsa_word
    }

    #[must_use]
    pub const fn content(self) -> PageContent {
        self.content
    }
}

impl<'a> DecodedPageEnvelope<'a> {
    #[must_use]
    pub const fn profile(&self) -> FormatProfile {
        self.profile
    }

    #[must_use]
    pub const fn id(&self) -> Vpid {
        self.id
    }

    #[must_use]
    pub const fn page_type(&self) -> PageType {
        self.page_type
    }

    #[must_use]
    pub const fn lsa_word(&self) -> u64 {
        self.lsa_word
    }

    #[must_use]
    pub const fn content(&self) -> PageContent {
        self.content
    }

    #[must_use]
    pub const fn tde_algorithm(&self) -> Option<TdeAlgorithm> {
        match self.content {
            PageContent::Plaintext => None,
            PageContent::Decrypted { algorithm } | PageContent::EncryptedOpaque { algorithm } => {
                Some(algorithm)
            }
        }
    }

    pub(crate) fn plaintext(&self, rule: &'static str) -> Result<ByteView<'a>, DecodeError> {
        match self.content {
            PageContent::Plaintext | PageContent::Decrypted { .. } => {
                self.plaintext.ok_or_else(|| {
                    DecodeError::new(DecodeErrorKind::ByteAccess, "page.envelope.plaintext_state")
                })
            }
            PageContent::EncryptedOpaque { .. } => {
                Err(DecodeError::new(DecodeErrorKind::EncryptedOpaque, rule))
            }
        }
    }
}

/// Attach an already decrypted, zeroizing user region to its validated
/// plaintext envelope. The original page must carry exactly one TDE flag;
/// callers cannot use this entry point to relabel plaintext or invalid flags.
pub fn decode_decrypted_page_envelope<'a>(
    encrypted_page: &[u8],
    decrypted_user: &'a [u8],
    expected: Vpid,
) -> Result<DecodedPageEnvelope<'a>, DecodeError> {
    decode_decrypted_page_envelope_with_profile(
        encrypted_page,
        decrypted_user,
        expected,
        FormatProfile::FeatOos,
    )
}

pub fn decode_decrypted_page_envelope_with_profile<'a>(
    encrypted_page: &[u8],
    decrypted_user: &'a [u8],
    expected: Vpid,
    profile: FormatProfile,
) -> Result<DecodedPageEnvelope<'a>, DecodeError> {
    if encrypted_page.len() != IO_PAGE_SIZE || decrypted_user.len() != DB_PAGE_SIZE {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidLength,
            "page.decrypted.physical_size",
        ));
    }
    let summary = decode_page_envelope_parts_with_profile(
        encrypted_page.get(..PAGE_PREFIX_SIZE).ok_or_else(|| {
            DecodeError::new(DecodeErrorKind::InvalidLength, "page.decrypted.prefix")
        })?,
        encrypted_page
            .get(IO_PAGE_SIZE - PAGE_WATERMARK_SIZE..)
            .ok_or_else(|| {
                DecodeError::new(DecodeErrorKind::InvalidLength, "page.decrypted.watermark")
            })?,
        expected,
        profile,
    )?;
    let PageContent::EncryptedOpaque { algorithm } = summary.content else {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidFlags,
            "page.decrypted.encrypted_source",
        ));
    };
    let page_offset = page_file_offset(expected)?;
    let user_offset = page_offset
        .checked_add(PAGE_PREFIX_SIZE as u64)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::ArithmeticOverflow,
                "page.decrypted.user_offset",
            )
        })?;
    Ok(DecodedPageEnvelope {
        id: summary.id,
        profile: summary.profile,
        page_type: summary.page_type,
        lsa_word: summary.lsa_word,
        content: PageContent::Decrypted { algorithm },
        plaintext: Some(ByteView::new(decrypted_user, user_offset)),
    })
}

pub fn decode_page_envelope(
    bytes: &[u8],
    expected: Vpid,
) -> Result<DecodedPageEnvelope<'_>, DecodeError> {
    decode_page_envelope_with_profile(bytes, expected, FormatProfile::FeatOos)
}

pub fn decode_page_envelope_with_profile(
    bytes: &[u8],
    expected: Vpid,
    profile: FormatProfile,
) -> Result<DecodedPageEnvelope<'_>, DecodeError> {
    if bytes.len() != IO_PAGE_SIZE {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidLength,
            "page.envelope.physical_size",
        ));
    }

    let summary = decode_page_envelope_parts_with_profile(
        bytes.get(..PAGE_PREFIX_SIZE).ok_or_else(|| {
            DecodeError::new(DecodeErrorKind::InvalidLength, "page.envelope.prefix_size")
        })?,
        bytes
            .get(IO_PAGE_SIZE - PAGE_WATERMARK_SIZE..)
            .ok_or_else(|| {
                DecodeError::new(
                    DecodeErrorKind::InvalidLength,
                    "page.envelope.watermark_size",
                )
            })?,
        expected,
        profile,
    )?;
    let page_offset = page_file_offset(expected)?;
    let view = ByteView::new(bytes, page_offset);
    let plaintext = match summary.content {
        PageContent::Plaintext => Some(
            view.subview(PAGE_PREFIX_SIZE, DB_PAGE_SIZE, "database page")
                .map_err(|_| {
                    DecodeError::new(DecodeErrorKind::ByteAccess, "page.envelope.user_region")
                })?,
        ),
        PageContent::EncryptedOpaque { .. } => None,
        PageContent::Decrypted { .. } => {
            unreachable!("fast envelope decoding cannot produce decrypted content")
        }
    };

    Ok(DecodedPageEnvelope {
        id: summary.id,
        profile: summary.profile,
        page_type: summary.page_type,
        lsa_word: summary.lsa_word,
        content: summary.content,
        plaintext,
    })
}

/// Decode the ordinary fast-scan envelope without reading the 16,344-byte
/// user region.
pub fn decode_page_envelope_parts(
    prefix: &[u8],
    watermark: &[u8],
    expected: Vpid,
) -> Result<PageEnvelopeSummary, DecodeError> {
    decode_page_envelope_parts_with_profile(prefix, watermark, expected, FormatProfile::FeatOos)
}

pub fn decode_page_envelope_parts_with_profile(
    prefix: &[u8],
    watermark: &[u8],
    expected: Vpid,
    profile: FormatProfile,
) -> Result<PageEnvelopeSummary, DecodeError> {
    if prefix.len() != PAGE_PREFIX_SIZE || watermark.len() != PAGE_WATERMARK_SIZE {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidLength,
            "page.envelope.parts_size",
        ));
    }

    let page_offset = page_file_offset(expected)?;
    let view = ByteView::new(prefix, page_offset);
    let watermark_offset = page_offset
        .checked_add((IO_PAGE_SIZE - PAGE_WATERMARK_SIZE) as u64)
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::ArithmeticOverflow,
                "page.envelope.watermark_offset",
            )
        })?;
    let watermark_view = ByteView::new(watermark, watermark_offset);

    let leading_lsa = view.read_u64_le(0, "page leading LSA").map_err(|_| {
        DecodeError::at(
            DecodeErrorKind::ByteAccess,
            "page.envelope.leading_lsa",
            page_offset,
        )
    })?;
    let trailing_lsa = watermark_view
        .read_u64_le(0, "page trailing LSA")
        .map_err(|_| {
            DecodeError::at(
                DecodeErrorKind::ByteAccess,
                "page.envelope.trailing_lsa",
                watermark_offset,
            )
        })?;
    if leading_lsa != trailing_lsa {
        return Err(DecodeError::new(
            DecodeErrorKind::LsaMismatch,
            "page.envelope.lsa_match",
        ));
    }

    let page_id = view
        .read_i32_le(8, "page identifier")
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, "page.envelope.page_id"))?;
    let vol_id = view
        .read_i16_le(12, "volume identifier")
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, "page.envelope.vol_id"))?;
    if page_id != expected.page_id.get() || vol_id != expected.vol_id.get() {
        return Err(DecodeError::new(
            DecodeErrorKind::IdentityMismatch,
            "page.envelope.identity_match",
        ));
    }

    let page_type = PageType::from_ordinal(
        view.read_u8(14, "physical page type")
            .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, "page.envelope.type"))?,
        profile,
    )?;
    let flags = view
        .read_u8(15, "page flags")
        .map_err(|_| DecodeError::new(DecodeErrorKind::ByteAccess, "page.envelope.flags"))?;
    if flags & !0x03 != 0 || flags == 0x03 {
        return Err(DecodeError::new(
            DecodeErrorKind::InvalidFlags,
            "page.envelope.tde_flags",
        ));
    }
    let content = match flags {
        0 => PageContent::Plaintext,
        0x01 => PageContent::EncryptedOpaque {
            algorithm: TdeAlgorithm::Aes,
        },
        0x02 => PageContent::EncryptedOpaque {
            algorithm: TdeAlgorithm::Aria,
        },
        _ => unreachable!("flags validated above"),
    };

    Ok(PageEnvelopeSummary {
        id: expected,
        profile,
        page_type,
        lsa_word: leading_lsa,
        content,
    })
}

fn page_file_offset(expected: Vpid) -> Result<u64, DecodeError> {
    u64::try_from(expected.page_id.get())
        .ok()
        .and_then(|page_id| page_id.checked_mul(IO_PAGE_SIZE as u64))
        .ok_or_else(|| {
            DecodeError::new(
                DecodeErrorKind::ArithmeticOverflow,
                "page.envelope.file_offset",
            )
        })
}
