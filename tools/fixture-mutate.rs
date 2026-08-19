//! Deterministic one-range fixture mutation utility.

use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

struct PendingOutput {
    path: PathBuf,
    committed: bool,
}

impl Drop for PendingOutput {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn parse_hex(value: &str) -> Result<Vec<u8>, String> {
    if value.is_empty() || !value.len().is_multiple_of(2) {
        return Err("hex value must contain a nonempty even number of digits".to_owned());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|digits| {
            let text = std::str::from_utf8(digits).map_err(|_| "hex value is not ASCII")?;
            u8::from_str_radix(text, 16).map_err(|_| "hex value contains a non-hex digit")
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(str::to_owned)
}

fn mutate(
    input_path: &Path,
    output_path: &Path,
    offset: u64,
    expected: &[u8],
    replacement: &[u8],
) -> Result<(), String> {
    if expected.len() != replacement.len() {
        return Err("expected and replacement ranges must have equal length".to_owned());
    }
    let mut input = File::open(input_path).map_err(|error| format!("open input: {error}"))?;
    let metadata = input
        .metadata()
        .map_err(|error| format!("inspect input: {error}"))?;
    if !metadata.is_file() {
        return Err("input must be a regular file".to_owned());
    }
    let range_length = u64::try_from(expected.len()).map_err(|_| "range length overflow")?;
    let range_end = offset
        .checked_add(range_length)
        .ok_or("mutation range overflow")?;
    if range_end > metadata.len() {
        return Err("mutation range exceeds input".to_owned());
    }

    input
        .seek(SeekFrom::Start(offset))
        .map_err(|error| format!("seek input: {error}"))?;
    let mut actual = vec![0; expected.len()];
    input
        .read_exact(&mut actual)
        .map_err(|error| format!("read expected range: {error}"))?;
    if actual != expected {
        return Err("input does not match the declared expected range".to_owned());
    }
    input
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("rewind input: {error}"))?;

    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output_path)
        .map_err(|error| format!("create output: {error}"))?;
    let mut pending = PendingOutput {
        path: output_path.to_owned(),
        committed: false,
    };
    io::copy(&mut Read::by_ref(&mut input).take(offset), &mut output)
        .map_err(|error| format!("copy prefix: {error}"))?;
    let mut discarded = vec![0; expected.len()];
    input
        .read_exact(&mut discarded)
        .map_err(|error| format!("read replaced range: {error}"))?;
    output
        .write_all(replacement)
        .map_err(|error| format!("write replacement: {error}"))?;
    io::copy(&mut input, &mut output).map_err(|error| format!("copy suffix: {error}"))?;
    output
        .sync_all()
        .map_err(|error| format!("sync output: {error}"))?;
    pending.committed = true;
    Ok(())
}

fn run() -> Result<(), String> {
    let arguments = env::args_os().collect::<Vec<_>>();
    let [_, input, output, offset, expected, replacement] = arguments.as_slice() else {
        return Err(
            "usage: fixture-mutate INPUT OUTPUT OFFSET EXPECTED_HEX REPLACEMENT_HEX".to_owned(),
        );
    };
    let offset = offset
        .to_str()
        .ok_or("offset is not UTF-8")?
        .parse::<u64>()
        .map_err(|_| "offset must be an unsigned decimal integer")?;
    let expected = parse_hex(expected.to_str().ok_or("expected hex is not UTF-8")?)?;
    let replacement = parse_hex(replacement.to_str().ok_or("replacement hex is not UTF-8")?)?;
    mutate(
        Path::new(input),
        Path::new(output),
        offset,
        &expected,
        &replacement,
    )?;
    println!("mutated offset={offset} length={}", expected.len());
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("fixture mutation failed: {error}");
        std::process::exit(2);
    }
}
