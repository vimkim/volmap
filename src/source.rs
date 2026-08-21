//! Stable read-only volume discovery and positional I/O.

use std::env;
use std::fmt;
use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::{Path, PathBuf};

use crate::format::{DB_PAGE_SIZE, IO_PAGE_SIZE, PAGE_PREFIX_SIZE, PAGE_WATERMARK_SIZE};
use crate::model::{PageId, VolId, Vpid};

const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputSpec {
    Database {
        name: String,
        databases_file: Option<PathBuf>,
    },
    Vinf {
        path: PathBuf,
        volume_root: Option<PathBuf>,
    },
}

impl InputSpec {
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Database { .. } => "database",
            Self::Vinf { .. } => "vinf",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileStamp {
    pub device: u64,
    pub inode: u64,
    pub length: u64,
    pub modified_seconds: i64,
    pub modified_nanoseconds: i64,
}

impl FileStamp {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
        }
    }
}

/// The ordered declared volume identities and file stamps observed for one
/// reading of an input, including volume-set membership so an added or removed
/// volume is a change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputFingerprint {
    input_kind: &'static str,
    volumes: Vec<(VolId, FileStamp)>,
}

impl InputFingerprint {
    #[must_use]
    pub const fn input_kind(&self) -> &'static str {
        self.input_kind
    }

    /// When the input last changed on disk, as a Unix second count.
    ///
    /// This is not when a transaction committed. A change committed to the log
    /// but not yet written to a data volume has moved no stamp here, so a
    /// reader comparing this against their own clock is seeing the engine's
    /// flush cadence rather than anything the reader can hurry along.
    #[must_use]
    pub fn newest_modified_unix_seconds(&self) -> Option<u64> {
        self.volumes
            .iter()
            .map(|(_, stamp)| stamp.modified_seconds)
            .max()
            .and_then(|seconds| u64::try_from(seconds).ok())
    }

    #[must_use]
    pub fn volumes(&self) -> &[(VolId, FileStamp)] {
        &self.volumes
    }
}

/// Reads the input's fingerprint manifest without opening or reading volume
/// pages. Volume-set membership comes from the manifest, so an extended
/// database is observed as a change rather than missed.
pub fn fingerprint(input: &InputSpec) -> Result<InputFingerprint, SourceError> {
    let (vinf_path, volume_root) = resolve_manifest(input)?;
    let entries = parse_vinf(&vinf_path, volume_root)?;
    let mut volumes = Vec::with_capacity(entries.len());
    for (declared_id, path) in entries {
        let metadata = path
            .metadata()
            .map_err(|error| SourceError::io_path("inspect volume", path.clone(), error))?;
        volumes.push((declared_id, FileStamp::from_metadata(&metadata)));
    }
    Ok(InputFingerprint {
        input_kind: input.kind(),
        volumes,
    })
}

pub struct VolumeHandle {
    declared_id: VolId,
    file: File,
    stamp: FileStamp,
}

impl fmt::Debug for VolumeHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VolumeHandle")
            .field("declared_id", &self.declared_id)
            .field("stamp", &self.stamp)
            .finish_non_exhaustive()
    }
}

impl VolumeHandle {
    #[must_use]
    pub const fn declared_id(&self) -> VolId {
        self.declared_id
    }

    #[must_use]
    pub const fn stamp(&self) -> FileStamp {
        self.stamp
    }

    pub fn current_stamp(&self) -> Result<FileStamp, SourceError> {
        self.file
            .metadata()
            .map(|metadata| FileStamp::from_metadata(&metadata))
            .map_err(|error| SourceError::io("inspect volume metadata", error))
    }

    pub fn is_unchanged(&self) -> Result<bool, SourceError> {
        self.current_stamp().map(|current| current == self.stamp)
    }

    pub fn read_page(&self, page_id: PageId) -> Result<Box<[u8; IO_PAGE_SIZE]>, SourceError> {
        let offset = page_offset(page_id)?;
        let mut page = Box::new([0_u8; IO_PAGE_SIZE]);
        read_exact_at(&self.file, page.as_mut_slice(), offset)?;
        Ok(page)
    }

    pub fn read_envelope(
        &self,
        page_id: PageId,
    ) -> Result<([u8; PAGE_PREFIX_SIZE], [u8; PAGE_WATERMARK_SIZE]), SourceError> {
        let offset = page_offset(page_id)?;
        let watermark_offset = offset
            .checked_add((IO_PAGE_SIZE - PAGE_WATERMARK_SIZE) as u64)
            .ok_or_else(|| SourceError::arithmetic("page watermark offset"))?;
        let mut prefix = [0_u8; PAGE_PREFIX_SIZE];
        let mut watermark = [0_u8; PAGE_WATERMARK_SIZE];
        read_exact_at(&self.file, &mut prefix, offset)?;
        read_exact_at(&self.file, &mut watermark, watermark_offset)?;
        Ok((prefix, watermark))
    }

    pub fn read_page_user_prefix<const N: usize>(
        &self,
        page_id: PageId,
    ) -> Result<[u8; N], SourceError> {
        if N > DB_PAGE_SIZE {
            return Err(SourceError::arithmetic("page user prefix length"));
        }
        let offset = page_offset(page_id)?
            .checked_add(PAGE_PREFIX_SIZE as u64)
            .ok_or_else(|| SourceError::arithmetic("page user prefix offset"))?;
        let mut prefix = [0_u8; N];
        read_exact_at(&self.file, &mut prefix, offset)?;
        Ok(prefix)
    }

    #[must_use]
    pub const fn vpid(&self, page_id: PageId) -> Vpid {
        Vpid::new(self.declared_id, page_id)
    }
}

#[derive(Debug)]
pub struct SourceSet {
    input_kind: &'static str,
    volumes: Vec<VolumeHandle>,
}

impl SourceSet {
    #[must_use]
    pub const fn input_kind(&self) -> &'static str {
        self.input_kind
    }

    #[must_use]
    pub fn volumes(&self) -> &[VolumeHandle] {
        &self.volumes
    }

    pub(crate) fn volume(&self, vol_id: VolId) -> Option<&VolumeHandle> {
        self.volumes
            .binary_search_by_key(&vol_id.get(), |volume| volume.declared_id.get())
            .ok()
            .and_then(|index| self.volumes.get(index))
    }

    /// The fingerprint manifest observed when this set was discovered.
    #[must_use]
    pub fn fingerprint(&self) -> InputFingerprint {
        InputFingerprint {
            input_kind: self.input_kind,
            volumes: self
                .volumes
                .iter()
                .map(|volume| (volume.declared_id, volume.stamp))
                .collect(),
        }
    }

    pub fn verify_unchanged(&self) -> Result<bool, SourceError> {
        for volume in &self.volumes {
            if !volume.is_unchanged()? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[derive(Debug)]
pub struct SourceError {
    kind: SourceErrorKind,
    action: &'static str,
    path: Option<PathBuf>,
    source: Option<io::Error>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceErrorKind {
    Io,
    ManifestTooLarge,
    InvalidDatabaseName,
    DatabaseNotFound,
    InvalidDatabasesFile,
    InvalidVinf,
    InvalidVolumePath,
    MissingPrimaryVolume,
    ArithmeticOverflow,
}

impl SourceError {
    fn simple(kind: SourceErrorKind, action: &'static str) -> Self {
        Self {
            kind,
            action,
            path: None,
            source: None,
        }
    }

    fn path(kind: SourceErrorKind, action: &'static str, path: PathBuf) -> Self {
        Self {
            kind,
            action,
            path: Some(path),
            source: None,
        }
    }

    fn io(action: &'static str, source: io::Error) -> Self {
        Self {
            kind: SourceErrorKind::Io,
            action,
            path: None,
            source: Some(source),
        }
    }

    fn io_path(action: &'static str, path: PathBuf, source: io::Error) -> Self {
        Self {
            kind: SourceErrorKind::Io,
            action,
            path: Some(path),
            source: Some(source),
        }
    }

    fn arithmetic(action: &'static str) -> Self {
        Self::simple(SourceErrorKind::ArithmeticOverflow, action)
    }

    #[must_use]
    pub const fn kind(&self) -> SourceErrorKind {
        self.kind
    }

    #[must_use]
    pub const fn action(&self) -> &'static str {
        self.action
    }

    #[must_use]
    pub fn path_value(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.action)?;
        if let Some(path) = &self.path {
            write!(formatter, " {}", path.display())?;
        }
        if let Some(source) = &self.source {
            write!(formatter, ": {source}")?;
        }
        Ok(())
    }
}

impl std::error::Error for SourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

fn resolve_manifest(input: &InputSpec) -> Result<(PathBuf, Option<&Path>), SourceError> {
    match input {
        InputSpec::Database {
            name,
            databases_file,
        } => Ok((resolve_database(name, databases_file.as_deref())?, None)),
        InputSpec::Vinf { path, volume_root } => Ok((path.clone(), volume_root.as_deref())),
    }
}

pub fn discover(input: &InputSpec) -> Result<SourceSet, SourceError> {
    let (vinf_path, volume_root) = resolve_manifest(input)?;
    let entries = parse_vinf(&vinf_path, volume_root)?;
    let mut volumes = Vec::with_capacity(entries.len());
    for (declared_id, path) in entries {
        let file = OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(|error| SourceError::io_path("open volume", path.clone(), error))?;
        let metadata = file
            .metadata()
            .map_err(|error| SourceError::io_path("inspect volume", path.clone(), error))?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() % IO_PAGE_SIZE as u64 != 0 {
            return Err(SourceError::path(
                SourceErrorKind::InvalidVolumePath,
                "volume is not a nonempty page-aligned regular file",
                path,
            ));
        }
        volumes.push(VolumeHandle {
            declared_id,
            file,
            stamp: FileStamp::from_metadata(&metadata),
        });
    }
    Ok(SourceSet {
        input_kind: input.kind(),
        volumes,
    })
}

fn resolve_database(name: &str, explicit: Option<&Path>) -> Result<PathBuf, SourceError> {
    if name.is_empty()
        || name
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte == b'/' || byte == b'\\')
    {
        return Err(SourceError::simple(
            SourceErrorKind::InvalidDatabaseName,
            "invalid database name",
        ));
    }
    let databases_file = explicit.map_or_else(default_databases_file, Path::to_path_buf);
    let content = read_small_text(&databases_file, "read databases file")?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = trimmed.split_ascii_whitespace().collect();
        if !(fields.len() == 4 || fields.len() == 5) {
            return Err(SourceError::path(
                SourceErrorKind::InvalidDatabasesFile,
                "invalid databases file entry",
                databases_file,
            ));
        }
        if fields[0] == name {
            return Ok(Path::new(fields[1]).join(format!("{name}_vinf")));
        }
    }
    Err(SourceError::path(
        SourceErrorKind::DatabaseNotFound,
        "database not found in databases file",
        databases_file,
    ))
}

fn default_databases_file() -> PathBuf {
    env::var_os("CUBRID_DATABASES").map_or_else(
        || PathBuf::from("databases.txt"),
        |directory| PathBuf::from(directory).join("databases.txt"),
    )
}

fn parse_vinf(
    path: &Path,
    volume_root: Option<&Path>,
) -> Result<Vec<(VolId, PathBuf)>, SourceError> {
    let content = read_small_text(path, "read volume manifest")?;
    let canonical_root = volume_root
        .map(|root| {
            root.canonicalize().map_err(|error| {
                SourceError::io_path("canonicalize volume root", root.to_path_buf(), error)
            })
        })
        .transpose()?;
    let mut last_id: Option<i32> = None;
    let mut entries: Vec<(VolId, PathBuf)> = Vec::new();
    for line in content.lines() {
        let fields: Vec<&str> = line.split_ascii_whitespace().collect();
        if fields.is_empty() {
            continue;
        }
        if fields.len() != 2 {
            return Err(SourceError::path(
                SourceErrorKind::InvalidVinf,
                "invalid volume manifest entry",
                path.to_path_buf(),
            ));
        }
        let raw_id = fields[0].parse::<i32>().map_err(|_| {
            SourceError::path(
                SourceErrorKind::InvalidVinf,
                "invalid volume identifier",
                path.to_path_buf(),
            )
        })?;
        if last_id.is_some_and(|previous| raw_id < previous) {
            return Err(SourceError::path(
                SourceErrorKind::InvalidVinf,
                "decreasing volume identifiers",
                path.to_path_buf(),
            ));
        }
        last_id = Some(raw_id);
        if raw_id < 0 {
            continue;
        }
        let narrowed = i16::try_from(raw_id).map_err(|_| {
            SourceError::path(
                SourceErrorKind::InvalidVinf,
                "volume identifier outside pinned range",
                path.to_path_buf(),
            )
        })?;
        let declared_id = VolId::new(narrowed).map_err(|_| {
            SourceError::path(
                SourceErrorKind::InvalidVinf,
                "negative data volume identifier",
                path.to_path_buf(),
            )
        })?;
        if entries
            .last()
            .is_some_and(|(previous, _)| previous.get() == declared_id.get())
        {
            return Err(SourceError::path(
                SourceErrorKind::InvalidVinf,
                "duplicate data volume identifier",
                path.to_path_buf(),
            ));
        }
        let recorded = Path::new(fields[1]);
        let resolved = if let Some(root) = &canonical_root {
            let basename = recorded.file_name().ok_or_else(|| {
                SourceError::path(
                    SourceErrorKind::InvalidVolumePath,
                    "volume entry has no basename",
                    recorded.to_path_buf(),
                )
            })?;
            let candidate = root.join(basename).canonicalize().map_err(|error| {
                SourceError::io_path("canonicalize remapped volume", root.join(basename), error)
            })?;
            if !candidate.starts_with(root) {
                return Err(SourceError::path(
                    SourceErrorKind::InvalidVolumePath,
                    "remapped volume escapes root",
                    candidate,
                ));
            }
            candidate
        } else {
            recorded.to_path_buf()
        };
        entries.push((declared_id, resolved));
    }
    if entries.first().map(|(id, _)| id.get()) != Some(0) {
        return Err(SourceError::path(
            SourceErrorKind::MissingPrimaryVolume,
            "volume manifest has no data volume zero",
            path.to_path_buf(),
        ));
    }
    Ok(entries)
}

fn read_small_text(path: &Path, action: &'static str) -> Result<String, SourceError> {
    let metadata = path
        .metadata()
        .map_err(|error| SourceError::io_path(action, path.to_path_buf(), error))?;
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err(SourceError::path(
            SourceErrorKind::ManifestTooLarge,
            "manifest exceeds size limit",
            path.to_path_buf(),
        ));
    }
    std::fs::read_to_string(path)
        .map_err(|error| SourceError::io_path(action, path.to_path_buf(), error))
}

fn page_offset(page_id: PageId) -> Result<u64, SourceError> {
    u64::try_from(page_id.get())
        .ok()
        .and_then(|page| page.checked_mul(IO_PAGE_SIZE as u64))
        .ok_or_else(|| SourceError::arithmetic("calculate page offset"))
}

fn read_exact_at(file: &File, mut buffer: &mut [u8], mut offset: u64) -> Result<(), SourceError> {
    while !buffer.is_empty() {
        match file.read_at(buffer, offset) {
            Ok(0) => {
                return Err(SourceError::io(
                    "read volume bytes",
                    io::Error::new(io::ErrorKind::UnexpectedEof, "short positional read"),
                ));
            }
            Ok(read) => {
                let (_, remaining) = buffer.split_at_mut(read);
                buffer = remaining;
                offset = offset
                    .checked_add(
                        u64::try_from(read)
                            .map_err(|_| SourceError::arithmetic("advance read offset"))?,
                    )
                    .ok_or_else(|| SourceError::arithmetic("advance read offset"))?;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(SourceError::io("read volume bytes", error)),
        }
    }
    Ok(())
}
