//! Offline TDE primitives for the pinned Linux x86-64 CUBRID profile.
//!
//! This module never serializes, logs, or exposes key material. It decodes the
//! native key-info/key-file layouts and owns the permanent data key in a
//! zeroizing, non-clonable container.

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use aes::Aes256;
use aria::Aria256;
use cipher::{KeyIvInit, StreamCipher};
use ctr::Ctr128BE;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

use crate::format::{DB_PAGE_SIZE, IO_PAGE_SIZE, PAGE_PREFIX_SIZE, TdeAlgorithm};

const KEY_FILE_MAGIC_SIZE: usize = 25;
const KEY_FILE_MAGIC: &[u8; KEY_FILE_MAGIC_SIZE] = b"CUBRID/Keys\0\0\0\0\0\0\0\0\0\0\0\0\0\0";
const KEY_FILE_ITEM_SIZE: usize = 40;
const KEY_FILE_ITEM_LIMIT: usize = 128;
const KEY_INFO_RECORD_SIZE: usize = 156;
const KEY_SIZE: usize = 32;
const PAGE_NONCE_PREFIX_OFFSET: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TdeErrorKind {
    Io,
    InvalidKeyFile,
    MissingMasterKey,
    MismatchedMasterKey,
    InvalidKeyInfo,
    InvalidPage,
    Cipher,
}

#[derive(Debug)]
pub struct TdeError {
    kind: TdeErrorKind,
    source: Option<io::Error>,
}

impl TdeError {
    const fn simple(kind: TdeErrorKind) -> Self {
        Self { kind, source: None }
    }

    fn io(source: io::Error) -> Self {
        Self {
            kind: TdeErrorKind::Io,
            source: Some(source),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> TdeErrorKind {
        self.kind
    }
}

impl fmt::Display for TdeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Deliberately omit the key-file path and all key identifiers.
        write!(formatter, "TDE key error: {:?}", self.kind)
    }
}

impl std::error::Error for TdeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

pub struct PermanentDataKey {
    bytes: [u8; KEY_SIZE],
}

impl fmt::Debug for PermanentDataKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PermanentDataKey { redacted }")
    }
}

impl Drop for PermanentDataKey {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

pub struct TdeKeyInfo {
    master_key_index: usize,
    created_time: i64,
    master_key_hash: [u8; KEY_SIZE],
    encrypted_permanent_key: [u8; KEY_SIZE],
}

impl fmt::Debug for TdeKeyInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TdeKeyInfo { redacted }")
    }
}

#[derive(Debug)]
pub struct LoadedTdeKey {
    pub key: PermanentDataKey,
    pub insecure_permissions: bool,
}

/// Decode the exact `sizeof(int) + sizeof(TDE_KEYINFO)` heap record.
pub fn decode_key_info_record(record: &[u8]) -> Result<TdeKeyInfo, TdeError> {
    if record.len() != KEY_INFO_RECORD_SIZE {
        return Err(TdeError::simple(TdeErrorKind::InvalidKeyInfo));
    }
    let dummy = i32::from_le_bytes(
        record[0..4]
            .try_into()
            .map_err(|_| TdeError::simple(TdeErrorKind::InvalidKeyInfo))?,
    );
    let raw_index = i32::from_le_bytes(
        record[4..8]
            .try_into()
            .map_err(|_| TdeError::simple(TdeErrorKind::InvalidKeyInfo))?,
    );
    let master_key_index = usize::try_from(raw_index)
        .ok()
        .filter(|index| *index < KEY_FILE_ITEM_LIMIT)
        .ok_or_else(|| TdeError::simple(TdeErrorKind::InvalidKeyInfo))?;
    let created_time = i64::from_le_bytes(
        record[12..20]
            .try_into()
            .map_err(|_| TdeError::simple(TdeErrorKind::InvalidKeyInfo))?,
    );
    if dummy != 0 || created_time < 0 {
        return Err(TdeError::simple(TdeErrorKind::InvalidKeyInfo));
    }
    let master_key_hash = record[28..60]
        .try_into()
        .map_err(|_| TdeError::simple(TdeErrorKind::InvalidKeyInfo))?;
    let encrypted_permanent_key = record[60..92]
        .try_into()
        .map_err(|_| TdeError::simple(TdeErrorKind::InvalidKeyInfo))?;
    Ok(TdeKeyInfo {
        master_key_index,
        created_time,
        master_key_hash,
        encrypted_permanent_key,
    })
}

/// Open and validate one explicitly supplied master-key file, then unwrap the
/// permanent data key. The complete key-file buffer and selected master key
/// are zeroized before return.
pub fn load_permanent_key(path: &Path, key_info: &TdeKeyInfo) -> Result<LoadedTdeKey, TdeError> {
    let mut file = File::open(path).map_err(TdeError::io)?;
    let metadata = file.metadata().map_err(TdeError::io)?;
    if !metadata.is_file() {
        return Err(TdeError::simple(TdeErrorKind::InvalidKeyFile));
    }
    let length = usize::try_from(metadata.len())
        .ok()
        .filter(|length| {
            *length >= KEY_FILE_MAGIC_SIZE
                && *length <= KEY_FILE_MAGIC_SIZE + KEY_FILE_ITEM_LIMIT * KEY_FILE_ITEM_SIZE
                && (*length - KEY_FILE_MAGIC_SIZE).is_multiple_of(KEY_FILE_ITEM_SIZE)
        })
        .ok_or_else(|| TdeError::simple(TdeErrorKind::InvalidKeyFile))?;
    let mut contents = Zeroizing::new(vec![0_u8; length]);
    file.read_exact(&mut contents).map_err(TdeError::io)?;
    if contents[..KEY_FILE_MAGIC_SIZE] != KEY_FILE_MAGIC[..] {
        return Err(TdeError::simple(TdeErrorKind::InvalidKeyFile));
    }
    let item_count = (length - KEY_FILE_MAGIC_SIZE) / KEY_FILE_ITEM_SIZE;
    if key_info.master_key_index >= item_count {
        return Err(TdeError::simple(TdeErrorKind::MissingMasterKey));
    }
    let item_offset = KEY_FILE_MAGIC_SIZE + key_info.master_key_index * KEY_FILE_ITEM_SIZE;
    let created_time = i64::from_le_bytes(
        contents[item_offset..item_offset + 8]
            .try_into()
            .map_err(|_| TdeError::simple(TdeErrorKind::InvalidKeyFile))?,
    );
    if created_time == -1 {
        return Err(TdeError::simple(TdeErrorKind::MissingMasterKey));
    }
    let mut master_key = Zeroizing::new([0_u8; KEY_SIZE]);
    master_key.copy_from_slice(&contents[item_offset + 8..item_offset + KEY_FILE_ITEM_SIZE]);
    let hash = Sha256::digest(master_key.as_slice());
    let matches = bool::from(hash.as_slice().ct_eq(&key_info.master_key_hash));
    if created_time != key_info.created_time || !matches {
        return Err(TdeError::simple(TdeErrorKind::MismatchedMasterKey));
    }

    let mut permanent_key = key_info.encrypted_permanent_key;
    apply_aes_ctr(master_key.as_slice(), &[0_u8; 16], &mut permanent_key)?;
    Ok(LoadedTdeKey {
        key: PermanentDataKey {
            bytes: permanent_key,
        },
        insecure_permissions: metadata.mode() & 0o077 != 0,
    })
}

/// Decrypt only the 16,344-byte database-page user region. The returned
/// buffer zeroizes on drop and never includes the plaintext prefix or nonce.
pub fn decrypt_page_user_region(
    page: &[u8],
    algorithm: TdeAlgorithm,
    key: &PermanentDataKey,
) -> Result<Zeroizing<Vec<u8>>, TdeError> {
    if page.len() != IO_PAGE_SIZE {
        return Err(TdeError::simple(TdeErrorKind::InvalidPage));
    }
    let mut nonce = [0_u8; 16];
    nonce[..8].copy_from_slice(
        page.get(PAGE_NONCE_PREFIX_OFFSET..PAGE_PREFIX_SIZE)
            .ok_or_else(|| TdeError::simple(TdeErrorKind::InvalidPage))?,
    );
    let mut plaintext = Zeroizing::new(
        page.get(PAGE_PREFIX_SIZE..PAGE_PREFIX_SIZE + DB_PAGE_SIZE)
            .ok_or_else(|| TdeError::simple(TdeErrorKind::InvalidPage))?
            .to_vec(),
    );
    match algorithm {
        TdeAlgorithm::Aes => apply_aes_ctr(&key.bytes, &nonce, &mut plaintext)?,
        TdeAlgorithm::Aria => apply_aria_ctr(&key.bytes, &nonce, &mut plaintext)?,
    }
    Ok(plaintext)
}

fn apply_aes_ctr(key: &[u8], nonce: &[u8; 16], bytes: &mut [u8]) -> Result<(), TdeError> {
    let mut cipher = Ctr128BE::<Aes256>::new_from_slices(key, nonce)
        .map_err(|_| TdeError::simple(TdeErrorKind::Cipher))?;
    cipher
        .try_apply_keystream(bytes)
        .map_err(|_| TdeError::simple(TdeErrorKind::Cipher))
}

fn apply_aria_ctr(key: &[u8], nonce: &[u8; 16], bytes: &mut [u8]) -> Result<(), TdeError> {
    let mut cipher = Ctr128BE::<Aria256>::new_from_slices(key, nonce)
        .map_err(|_| TdeError::simple(TdeErrorKind::Cipher))?;
    cipher
        .try_apply_keystream(bytes)
        .map_err(|_| TdeError::simple(TdeErrorKind::Cipher))
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use sha2::{Digest, Sha256};

    use crate::format::{DB_PAGE_SIZE, IO_PAGE_SIZE, TdeAlgorithm};

    use super::{
        KEY_FILE_MAGIC, PermanentDataKey, TdeErrorKind, apply_aes_ctr, apply_aria_ctr,
        decode_key_info_record, decrypt_page_user_region, load_permanent_key,
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempKeyFile(PathBuf);

    impl TempKeyFile {
        fn new(bytes: &[u8]) -> Self {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("volmap-tde-test-{}-{sequence}", std::process::id()));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .unwrap();
            file.write_all(bytes).unwrap();
            Self(path)
        }
    }

    impl Drop for TempKeyFile {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).unwrap();
        }
    }

    fn key_info_record(
        index: i32,
        created_time: i64,
        master_key: &[u8; 32],
        permanent_key: &[u8; 32],
    ) -> [u8; 156] {
        let mut record = [0_u8; 156];
        record[4..8].copy_from_slice(&index.to_le_bytes());
        record[12..20].copy_from_slice(&created_time.to_le_bytes());
        record[28..60].copy_from_slice(&Sha256::digest(master_key));
        let mut encrypted = *permanent_key;
        apply_aes_ctr(master_key, &[0_u8; 16], &mut encrypted).unwrap();
        record[60..92].copy_from_slice(&encrypted);
        record
    }

    fn key_file(created_time: i64, master_key: &[u8; 32]) -> Vec<u8> {
        let mut bytes = KEY_FILE_MAGIC.to_vec();
        bytes.extend_from_slice(&created_time.to_le_bytes());
        bytes.extend_from_slice(master_key);
        bytes
    }

    #[test]
    fn aes_256_ctr_matches_nist_sp_800_38a_vector() {
        let key = [
            0x60, 0x3d, 0xeb, 0x10, 0x15, 0xca, 0x71, 0xbe, 0x2b, 0x73, 0xae, 0xf0, 0x85, 0x7d,
            0x77, 0x81, 0x1f, 0x35, 0x2c, 0x07, 0x3b, 0x61, 0x08, 0xd7, 0x2d, 0x98, 0x10, 0xa3,
            0x09, 0x14, 0xdf, 0xf4,
        ];
        let nonce = [
            0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xfb, 0xfc, 0xfd,
            0xfe, 0xff,
        ];
        let mut block = [
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a,
        ];
        apply_aes_ctr(&key, &nonce, &mut block).unwrap();
        assert_eq!(
            block,
            [
                0x60, 0x1e, 0xc3, 0x13, 0x77, 0x57, 0x89, 0xa5, 0xb7, 0xa7, 0xf5, 0x04, 0xbb, 0xf3,
                0xd2, 0x28,
            ]
        );
    }

    #[test]
    fn aria_256_ctr_matches_openssl_vector() {
        let key: [u8; 32] = core::array::from_fn(|index| u8::try_from(index).unwrap());
        let nonce = [0_u8; 16];
        let mut block = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff,
        ];
        apply_aria_ctr(&key, &nonce, &mut block).unwrap();
        assert_eq!(
            block,
            [
                0x62, 0x8c, 0xe5, 0xee, 0x72, 0x36, 0x67, 0xcf, 0xd4, 0xfc, 0x07, 0xcb, 0x4f, 0xfa,
                0xca, 0x05,
            ]
        );
    }

    #[test]
    fn matching_native_key_file_unwraps_only_the_permanent_key() {
        let master_key = [0x35_u8; 32];
        let permanent_key = [0xa7_u8; 32];
        let record = key_info_record(0, 123, &master_key, &permanent_key);
        let info = decode_key_info_record(&record).unwrap();
        let file = TempKeyFile::new(&key_file(123, &master_key));
        let loaded = load_permanent_key(&file.0, &info).unwrap();
        assert!(!loaded.insecure_permissions);
        assert_eq!(loaded.key.bytes, permanent_key);
        assert_eq!(format!("{:?}", loaded.key), "PermanentDataKey { redacted }");
        assert_eq!(format!("{info:?}"), "TdeKeyInfo { redacted }");
    }

    #[test]
    fn key_file_and_key_info_fail_closed_without_disclosing_path() {
        let master_key = [0x35_u8; 32];
        let permanent_key = [0xa7_u8; 32];
        let record = key_info_record(1, 123, &master_key, &permanent_key);
        let info = decode_key_info_record(&record).unwrap();
        let file = TempKeyFile::new(&key_file(123, &master_key));
        let error = load_permanent_key(&file.0, &info).unwrap_err();
        assert_eq!(error.kind(), TdeErrorKind::MissingMasterKey);
        assert!(
            !error
                .to_string()
                .contains(file.0.to_string_lossy().as_ref())
        );

        let mut invalid_magic = key_file(123, &master_key);
        invalid_magic[0] ^= 0xff;
        let file = TempKeyFile::new(&invalid_magic);
        let info =
            decode_key_info_record(&key_info_record(0, 123, &master_key, &permanent_key)).unwrap();
        assert_eq!(
            load_permanent_key(&file.0, &info).unwrap_err().kind(),
            TdeErrorKind::InvalidKeyFile
        );

        let mismatched = TempKeyFile::new(&key_file(123, &[0x36_u8; 32]));
        assert_eq!(
            load_permanent_key(&mismatched.0, &info).unwrap_err().kind(),
            TdeErrorKind::MismatchedMasterKey
        );

        let mut invalid_record = key_info_record(0, 123, &master_key, &permanent_key);
        invalid_record[0] = 1;
        assert_eq!(
            decode_key_info_record(&invalid_record).unwrap_err().kind(),
            TdeErrorKind::InvalidKeyInfo
        );
    }

    #[test]
    fn page_decryption_uses_stored_nonce_for_aes_and_aria() {
        let key = PermanentDataKey { bytes: [0x42; 32] };
        for algorithm in [TdeAlgorithm::Aes, TdeAlgorithm::Aria] {
            let mut page = [0_u8; IO_PAGE_SIZE];
            page[24..32].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
            let expected: Vec<u8> = (0..DB_PAGE_SIZE)
                .map(|index| u8::try_from(index % 251).unwrap())
                .collect();
            page[32..32 + DB_PAGE_SIZE].copy_from_slice(&expected);
            let mut nonce = [0_u8; 16];
            nonce[..8].copy_from_slice(&page[24..32]);
            match algorithm {
                TdeAlgorithm::Aes => {
                    apply_aes_ctr(&key.bytes, &nonce, &mut page[32..32 + DB_PAGE_SIZE]).unwrap();
                }
                TdeAlgorithm::Aria => {
                    apply_aria_ctr(&key.bytes, &nonce, &mut page[32..32 + DB_PAGE_SIZE]).unwrap();
                }
            }
            let decrypted = decrypt_page_user_region(&page, algorithm, &key).unwrap();
            assert_eq!(decrypted.as_slice(), expected);
        }
    }

    #[test]
    fn key_file_must_be_regular_and_layout_exact() {
        let master_key = [0x35_u8; 32];
        let permanent_key = [0xa7_u8; 32];
        let info =
            decode_key_info_record(&key_info_record(0, 123, &master_key, &permanent_key)).unwrap();
        let file = TempKeyFile::new(&KEY_FILE_MAGIC[..]);
        assert_eq!(
            load_permanent_key(&file.0, &info).unwrap_err().kind(),
            TdeErrorKind::MissingMasterKey
        );
        let file = TempKeyFile::new(&[0_u8; 26]);
        assert_eq!(
            load_permanent_key(&file.0, &info).unwrap_err().kind(),
            TdeErrorKind::InvalidKeyFile
        );
    }
}
