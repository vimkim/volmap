//! Incremental producer v1 decoder. Only a validated footer publishes evidence.
use super::memory::{Budget, Charge, MIB};
use serde::Serialize;
use serde_json::Value;

const RECORD_LIMIT: usize = 65_536;
const SCAN_LIMIT: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct VolumeIdentity {
    pub volid: u16,
    pub volume_creation: String,
    pub device: String,
    pub inode: String,
}

#[derive(Clone, Debug)]
pub(super) struct Hello {
    pub minor: u32,
    pub incarnation: String,
    pub database_creation: String,
    pub volumes: Vec<VolumeIdentity>,
    pub shared: u32,
    pub private: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub(super) enum Field<T> {
    Unknown,
    Empty,
    Known(T),
}
impl<T> Field<T> {
    pub(super) fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct Record {
    pub volid: u16,
    pub pageid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latch_mode: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiter_present: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flushing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub async_flush_requested: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_vacuum: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lru_zone: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lru_list_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Field::is_unknown")]
    pub lru_list_index: Field<u32>,
    #[serde(skip_serializing_if = "Field::is_unknown")]
    pub page_lsa: Field<Lsa>,
    #[serde(skip_serializing_if = "Field::is_unknown")]
    pub oldest_unflush_lsa: Field<Lsa>,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct Lsa {
    #[serde(serialize_with = "serialize_decimal")]
    pub pageid: u64,
    pub offset: u16,
}

#[derive(Debug)]
pub(super) struct Capture {
    _charge: Charge,
    pub sequence: u64,
    pub start_time_us: String,
    pub end_time_us: String,
    pub truncated: bool,
    pub records: Vec<Record>,
}

impl Capture {
    pub fn record(&self, volid: u16, pageid: u32) -> Result<Option<&Record>, ()> {
        let key = (volid, pageid);
        let start = self
            .records
            .partition_point(|record| (record.volid, record.pageid) < key);
        let end = self
            .records
            .partition_point(|record| (record.volid, record.pageid) <= key);
        match end - start {
            0 => Ok(None),
            1 => Ok(self.records.get(start)),
            _ => Err(()),
        }
    }

    #[cfg(test)]
    pub fn lookup(&self, volid: u16, pageid: u32, evaluated: bool) -> &'static str {
        if !evaluated {
            return "unknown";
        }
        match self.record(volid, pageid) {
            Ok(Some(_)) => "resident",
            Ok(None) if !self.truncated => "not-resident",
            _ => "unknown",
        }
    }
}

pub(super) struct Decoder {
    budget: Budget,
    _scratch: Charge,
    frame: Vec<u8>,
    pub hello: Option<Hello>,
    assembly: Option<Capture>,
    capture: Option<Capture>,
    scan_bytes: usize,
    sequence: u64,
    requested: bool,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::with_budget(Budget::default()).expect("initial decoder budget")
    }
}
impl Decoder {
    pub fn with_budget(budget: Budget) -> Result<Self, &'static str> {
        let scratch = budget.reserve(16 * MIB)?;
        Ok(Self {
            budget,
            _scratch: scratch,
            frame: Vec::with_capacity(65_536),
            hello: None,
            assembly: None,
            capture: None,
            scan_bytes: 0,
            sequence: 0,
            requested: false,
        })
    }

    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn set_sequence_floor(&mut self, floor: u64) {
        self.sequence = self.sequence.max(floor);
    }

    pub fn begin_scan(&mut self) -> Result<(), &'static str> {
        if self.hello.is_none() || !self.idle() || self.requested || self.capture.is_some() {
            return Err("ordering");
        }
        self.requested = true;
        Ok(())
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        for byte in bytes {
            let limit = if self.hello.is_none() { 65_536 } else { 4096 };
            if self.frame.len() >= limit {
                return Err("frame-limit");
            }
            self.frame.push(*byte);
            if *byte == b'\n' {
                let frame = std::mem::take(&mut self.frame);
                let result = self.accept(&frame);
                self.frame = frame;
                self.frame.clear();
                result?;
            }
        }
        Ok(())
    }

    pub fn capture(&self) -> Option<&Capture> {
        self.capture.as_ref()
    }
    pub fn take_capture(&mut self) -> Option<Capture> {
        self.capture.take()
    }
    pub fn idle(&self) -> bool {
        self.assembly.is_none() && self.frame.is_empty()
    }

    fn accept(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        let value: Value = parse(bytes)?;
        let kind = value
            .get("type")
            .and_then(Value::as_str)
            .ok_or("missing-type")?;
        if kind != "server_hello" && bytes.len() > 4096 {
            return Err("frame-limit");
        }
        match kind {
            "server_hello" if self.hello.is_none() => {
                self.hello = Some(parse_hello(&value)?);
            }
            "scan_header" if self.hello.is_some() && self.assembly.is_none() && self.requested => {
                let sequence = self.binding(&value)?;
                if sequence <= self.sequence {
                    return Err("sequence-mismatch");
                }
                self.sequence = sequence;
                self.scan_bytes = bytes.len();
                let charge = self.budget.reserve(32 * MIB)?;
                self.assembly = Some(Capture {
                    _charge: charge,
                    sequence,
                    start_time_us: decimal(&value, "start_time_us", u64::MAX)?.to_string(),
                    end_time_us: String::new(),
                    truncated: false,
                    records: Vec::with_capacity(RECORD_LIMIT),
                });
            }
            "page" | "scan_footer" if self.assembly.is_some() => {
                let sequence = self.binding(&value)?;
                self.scan_bytes = self
                    .scan_bytes
                    .checked_add(bytes.len())
                    .ok_or("scan-limit")?;
                if self.scan_bytes > SCAN_LIMIT {
                    return Err("scan-limit");
                }
                let capture = self.assembly.as_mut().ok_or("ordering")?;
                if sequence != capture.sequence {
                    return Err("sequence-mismatch");
                }
                if kind == "page" {
                    if capture.records.len() == RECORD_LIMIT {
                        return Err("record-limit");
                    }
                    capture
                        .records
                        .push(record(&value, self.hello.as_ref().ok_or("ordering")?)?);
                } else {
                    if integer(&value, "record_count", RECORD_LIMIT as u64)?
                        != capture.records.len() as u64
                        || integer(&value, "visited_slots", RECORD_LIMIT as u64)?
                            < capture.records.len() as u64
                    {
                        return Err("count-mismatch");
                    }
                    capture.truncated = value
                        .get("truncated")
                        .and_then(Value::as_bool)
                        .ok_or("missing-truncation")?;
                    capture.end_time_us = decimal(&value, "end_time_us", u64::MAX)?.to_string();
                    capture
                        .records
                        .sort_unstable_by_key(|record| (record.volid, record.pageid));
                    self.capture = self.assembly.take();
                    self.requested = false;
                }
            }
            "error" => {
                if self.assembly.is_some() {
                    return Err("ordering");
                }
                self.requested = false;
                return Err(match value.get("code").and_then(Value::as_str) {
                    Some("version-unsupported") => "version-unsupported",
                    Some("identity-oversized") => "identity-oversized",
                    Some("incarnation-changed") => "incarnation-changed",
                    Some("busy") => "producer-busy",
                    Some("rate-limited") => "rate-limited",
                    Some("parameter-off") => "parameter-off",
                    _ => "producer-refused",
                });
            }
            "client_hello" | "scan_request" | "server_hello" | "scan_header" | "page"
            | "scan_footer" => return Err("ordering"),
            _ if self.assembly.is_some() => {
                self.scan_bytes = self
                    .scan_bytes
                    .checked_add(bytes.len())
                    .ok_or("scan-limit")?;
                if self.scan_bytes > SCAN_LIMIT {
                    return Err("scan-limit");
                }
            }
            _ => return Err("ordering"),
        }
        Ok(())
    }

    fn binding(&self, value: &Value) -> Result<u64, &'static str> {
        if incarnation(value)? != self.hello.as_ref().ok_or("ordering")?.incarnation {
            return Err("incarnation-changed");
        }
        let sequence = decimal(value, "scan_seq", u64::MAX)?;
        if sequence == 0 {
            return Err("sequence-mismatch");
        }
        Ok(sequence)
    }
}

fn integer(value: &Value, key: &str, maximum: u64) -> Result<u64, &'static str> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|number| *number <= maximum)
        .ok_or("invalid-integer")
}

fn decimal(value: &Value, key: &str, maximum: u64) -> Result<u64, &'static str> {
    let text = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or("invalid-decimal")?;
    if text.is_empty()
        || text.len() > 20
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err("invalid-decimal");
    }
    text.parse::<u64>()
        .ok()
        .filter(|number| *number <= maximum)
        .ok_or("invalid-decimal")
}

fn incarnation(value: &Value) -> Result<String, &'static str> {
    let text = value
        .get("incarnation")
        .and_then(Value::as_str)
        .ok_or("invalid-incarnation")?;
    if text.len() != 32
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("invalid-incarnation");
    }
    Ok(text.to_owned())
}

fn semantic(
    value: &Value,
    key: &str,
    vocabulary: &[&'static str],
) -> Result<Option<&'static str>, &'static str> {
    match value.get(key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(
            vocabulary
                .iter()
                .copied()
                .find(|known| *known == text)
                .unwrap_or("unknown"),
        )),
        _ => Err("invalid-state"),
    }
}

fn boolean(value: &Value, key: &str) -> Result<Option<bool>, &'static str> {
    value
        .get(key)
        .map(|field| field.as_bool().ok_or("invalid-state"))
        .transpose()
}

fn lsa(value: &Value, key: &str) -> Result<Field<Lsa>, &'static str> {
    match value.get(key) {
        None => Ok(Field::Unknown),
        Some(Value::Null) => Ok(Field::Empty),
        Some(lsa) => Ok(Field::Known(Lsa {
            pageid: decimal(lsa, "pageid", i64::MAX as u64)?,
            offset: u16::try_from(integer(lsa, "offset", 32767)?).map_err(|_| "invalid-integer")?,
        })),
    }
}

fn record(value: &Value, hello: &Hello) -> Result<Record, &'static str> {
    let lru_zone = semantic(
        value,
        "lru_zone",
        &["lru1", "lru2", "lru3", "void", "invalid"],
    )?;
    let lru_list_kind = semantic(
        value,
        "lru_list_kind",
        &["shared", "private", "none", "invalid"],
    )?;
    let lru_list_index = match value.get("lru_list_index") {
        None | Some(Value::Null) => None,
        Some(_) => Some(
            u32::try_from(integer(value, "lru_list_index", i32::MAX as u64)?)
                .map_err(|_| "invalid-integer")?,
        ),
    };
    if value.get("lru_list_index").is_some() {
        let valid = match lru_list_kind {
            Some("shared") => lru_list_index.is_some_and(|index| index < hello.shared),
            Some("private") => lru_list_index.is_some_and(|index| index < hello.private),
            Some("none" | "invalid") => lru_list_index.is_none(),
            _ => true,
        };
        if !valid {
            return Err("invalid-lru");
        }
    }
    if matches!(lru_zone, Some("lru1" | "lru2" | "lru3")) && lru_list_kind == Some("none")
        || matches!(lru_zone, Some("void" | "invalid"))
            && matches!(lru_list_kind, Some("shared" | "private" | "invalid"))
    {
        return Err("invalid-lru");
    }
    Ok(Record {
        volid: u16::try_from(integer(value, "volid", 32767)?).map_err(|_| "invalid-integer")?,
        pageid: u32::try_from(integer(value, "pageid", i32::MAX as u64)?)
            .map_err(|_| "invalid-integer")?,
        page_kind: semantic(
            value,
            "page_kind",
            &[
                "unknown",
                "ftab",
                "heap",
                "volheader",
                "volbitmap",
                "qresult",
                "ehash",
                "overflow",
                "oos",
                "area",
                "catalog",
                "btree",
                "log",
                "dropped_files",
                "vacuum_data",
            ],
        )?,
        latch_mode: semantic(
            value,
            "latch_mode",
            &["none", "read", "write", "flush", "unknown"],
        )?,
        waiter_present: boolean(value, "waiter_present")?,
        fix_count: value
            .get("fix_count")
            .map(|_| {
                integer(value, "fix_count", i32::MAX as u64)
                    .and_then(|count| u32::try_from(count).map_err(|_| "invalid-integer"))
            })
            .transpose()?,
        dirty: boolean(value, "dirty")?,
        flushing: boolean(value, "flushing")?,
        async_flush_requested: boolean(value, "async_flush_requested")?,
        to_vacuum: boolean(value, "to_vacuum")?,
        lru_zone,
        lru_list_kind,
        lru_list_index: match lru_list_index {
            Some(index) => Field::Known(index),
            None if value.get("lru_list_index").is_some() => Field::Empty,
            None => Field::Unknown,
        },
        page_lsa: lsa(value, "page_lsa")?,
        oldest_unflush_lsa: lsa(value, "oldest_unflush_lsa")?,
    })
}

// RawValue validates syntax without imposing machine numeric range on ignored
// fields. Every subtree still receives depth and duplicate-name validation.
#[derive(Clone, Copy, PartialEq)]
enum Context {
    Frame,
    Volume,
    Volumes,
    Lsa,
    Scalar,
    Unknown,
}

struct RawObject<'a>(std::collections::BTreeMap<String, &'a serde_json::value::RawValue>);
struct RawObjectVisitor;
impl<'de> serde::Deserialize<'de> for RawObject<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(RawObjectVisitor)
    }
}
impl<'de> serde::de::Visitor<'de> for RawObjectVisitor {
    type Value = RawObject<'de>;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("an object without duplicate names")
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        mut access: A,
    ) -> Result<Self::Value, A::Error> {
        let mut fields = std::collections::BTreeMap::new();
        while let Some(key) = access.next_key::<String>()? {
            if fields.contains_key(&key) {
                return Err(serde::de::Error::custom("duplicate member"));
            }
            fields.insert(key, access.next_value::<&serde_json::value::RawValue>()?);
        }
        Ok(RawObject(fields))
    }
}

pub(super) fn parse(bytes: &[u8]) -> Result<Value, &'static str> {
    let raw: &serde_json::value::RawValue =
        serde_json::from_slice(bytes).map_err(|_| "malformed-json")?;
    let value = project(raw, 0, Context::Frame)?;
    if !value.is_object() {
        return Err("invalid-frame");
    }
    Ok(value)
}

fn project(
    raw: &serde_json::value::RawValue,
    depth: u8,
    context: Context,
) -> Result<Value, &'static str> {
    let text = raw.get();
    match text.as_bytes().first() {
        Some(b'{') => project_object(text, depth, context),
        Some(b'[') => {
            if depth >= 16 {
                return Err("depth-limit");
            }
            let elements: Vec<&serde_json::value::RawValue> =
                serde_json::from_str(text).map_err(|_| "malformed-json")?;
            let child = match context {
                Context::Volumes => Context::Volume,
                Context::Unknown => Context::Unknown,
                _ => Context::Scalar,
            };
            let mut result = Vec::new();
            for element in elements {
                let value = project(element, depth + 1, child)?;
                if context != Context::Unknown {
                    result.push(value);
                }
            }
            Ok(if context == Context::Unknown {
                Value::Null
            } else {
                Value::Array(result)
            })
        }
        _ if context == Context::Unknown => Ok(Value::Null),
        _ => serde_json::from_str(text).map_err(|_| "invalid-state"),
    }
}

fn project_object(text: &str, depth: u8, context: Context) -> Result<Value, &'static str> {
    if depth >= 16 {
        return Err("depth-limit");
    }
    let RawObject(fields) = serde_json::from_str(text).map_err(|_| "malformed-json")?;
    let kind: String = if context == Context::Frame {
        serde_json::from_str(fields.get("type").ok_or("missing-type")?.get())
            .map_err(|_| "missing-type")?
    } else {
        String::new()
    };
    let mut result = serde_json::Map::new();
    for (key, raw) in fields {
        let child = field_context(context, &kind, &key);
        let value = project(raw, depth + 1, child)?;
        if child != Context::Unknown {
            result.insert(key, value);
        }
    }
    Ok(if context == Context::Unknown {
        Value::Null
    } else {
        Value::Object(result)
    })
}

fn field_context(context: Context, kind: &str, key: &str) -> Context {
    let fields: &[&str] = match context {
        Context::Frame => match kind {
            "server_hello" => &[
                "protocol_major",
                "protocol_minor",
                "incarnation",
                "database_creation",
                "volumes",
                "shared_lru_count",
                "private_lru_count",
            ],
            "scan_header" => &["incarnation", "scan_seq", "start_time_us"],
            "scan_footer" => &[
                "incarnation",
                "scan_seq",
                "end_time_us",
                "record_count",
                "visited_slots",
                "truncated",
            ],
            "page" => &[
                "incarnation",
                "scan_seq",
                "volid",
                "pageid",
                "page_kind",
                "latch_mode",
                "waiter_present",
                "fix_count",
                "dirty",
                "flushing",
                "async_flush_requested",
                "to_vacuum",
                "lru_zone",
                "lru_list_kind",
                "lru_list_index",
                "page_lsa",
                "oldest_unflush_lsa",
            ],
            "error" => &["code", "supported_majors", "retry_after_ms"],
            "client_hello" => &["supported_majors", "expected_incarnation"],
            "scan_request" => &["incarnation"],
            _ => &[],
        },
        Context::Volume => &["volid", "volume_creation", "device", "inode"],
        Context::Lsa => &["pageid", "offset"],
        _ => &[],
    };
    if context == Context::Frame && key == "type" {
        return Context::Scalar;
    }
    if !fields.contains(&key) {
        return Context::Unknown;
    }
    match key {
        "volumes" => Context::Volumes,
        "page_lsa" | "oldest_unflush_lsa" => Context::Lsa,
        _ => Context::Scalar,
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde serialize_with requires a reference"
)]
fn serialize_decimal<S: serde::Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

// Records contain only scalars and static vocabulary: no hidden per-record
// allocations. Two simultaneous 16 MiB record slabs cover latest + assembly.
const _: () = assert!(std::mem::size_of::<Record>() <= 256);

fn parse_hello(value: &Value) -> Result<Hello, &'static str> {
    if integer(value, "protocol_major", i32::MAX as u64)? != 1 {
        return Err("version-unsupported");
    }
    let minor = u32::try_from(integer(value, "protocol_minor", i32::MAX as u64)?)
        .map_err(|_| "invalid-integer")?;
    let incarnation = incarnation(value)?;
    let database_creation = decimal(value, "database_creation", u64::MAX)?.to_string();
    let shared = u32::try_from(integer(value, "shared_lru_count", i32::MAX as u64)?)
        .map_err(|_| "invalid-integer")?;
    let private = u32::try_from(integer(value, "private_lru_count", i32::MAX as u64)?)
        .map_err(|_| "invalid-integer")?;
    let values = value
        .get("volumes")
        .and_then(Value::as_array)
        .ok_or("missing-volumes")?;
    if values.is_empty() {
        return Err("missing-volumes");
    }
    let mut volumes = Vec::with_capacity(values.len());
    for volume in values {
        volumes.push(VolumeIdentity {
            volid: u16::try_from(integer(volume, "volid", 32767)?)
                .map_err(|_| "invalid-integer")?,
            volume_creation: decimal(volume, "volume_creation", u64::MAX)?.to_string(),
            device: decimal(volume, "device", u64::MAX)?.to_string(),
            inode: decimal(volume, "inode", u64::MAX)?.to_string(),
        });
    }
    volumes.sort_unstable_by_key(|volume| volume.volid);
    if volumes
        .windows(2)
        .any(|pair| pair[0].volid == pair[1].volid)
    {
        return Err("duplicate-volume");
    }
    Ok(Hello {
        minor,
        incarnation,
        database_creation,
        volumes,
        shared,
        private,
    })
}
