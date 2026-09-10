//! Authenticated Linux stream adapter. All errors are stable disclosure codes.
use super::memory::Budget;
use super::wire::{Capture, Decoder, Hello};
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::net::UnixStream;

pub(super) struct Connection {
    stream: UnixStream,
    decoder: Decoder,
}

impl Connection {
    #[cfg(test)]
    pub async fn connect(path: &Path) -> Result<Self, &'static str> {
        Self::with_budget(path, Budget::default()).await
    }

    pub async fn with_budget(path: &Path, budget: Budget) -> Result<Self, &'static str> {
        let start = Instant::now();
        let result =
            tokio::time::timeout(Duration::from_millis(500), Self::handshake(path, budget))
                .await
                .map_err(|_| "attachment-timeout")?;
        if start.elapsed() >= Duration::from_millis(500) {
            return Err("attachment-timeout");
        }
        result
    }

    async fn handshake(path: &Path, budget: Budget) -> Result<Self, &'static str> {
        let uid = effective_uid()?;
        let directory = path.parent().ok_or("unsafe-socket")?;
        let parent = std::fs::symlink_metadata(directory).map_err(|_| "source-unavailable")?;
        let socket = std::fs::symlink_metadata(path).map_err(|_| "source-unavailable")?;
        if !parent.is_dir()
            || parent.uid() != uid
            || parent.mode() & 0o7777 != 0o700
            || !socket.file_type().is_socket()
            || socket.uid() != uid
            || socket.mode() & 0o7777 != 0o600
        {
            return Err("peer-refused");
        }
        let stream = UnixStream::connect(path)
            .await
            .map_err(|_| "source-unavailable")?;
        let peer = stream.peer_cred().map_err(|_| "peer-unverifiable")?;
        if peer.uid() != uid {
            return Err("peer-refused");
        }
        let current = std::fs::symlink_metadata(path).map_err(|_| "peer-refused")?;
        if current.dev() != socket.dev()
            || current.ino() != socket.ino()
            || current.uid() != uid
            || current.mode() & 0o7777 != 0o600
        {
            return Err("peer-refused");
        }
        let mut connection = Self {
            stream,
            decoder: Decoder::with_budget(budget)?,
        };
        connection
            .write(b"{\"type\":\"client_hello\",\"supported_majors\":[1]}\n")
            .await?;
        while connection.decoder.hello.is_none() {
            connection.read().await?;
        }
        if !connection.decoder.idle() || connection.decoder.capture().is_some() {
            return Err("protocol-invalid");
        }
        Ok(connection)
    }

    pub fn hello(&self) -> &Hello {
        self.decoder.hello.as_ref().expect("validated handshake")
    }

    pub fn sequence(&self) -> u64 {
        self.decoder.sequence()
    }
    pub fn set_sequence_floor(&mut self, floor: u64) {
        self.decoder.set_sequence_floor(floor);
    }

    pub async fn scan(&mut self) -> Result<Capture, &'static str> {
        let start = Instant::now();
        let result = tokio::time::timeout(Duration::from_secs(2), self.exchange())
            .await
            .map_err(|_| "scan-timeout")?;
        if start.elapsed() >= Duration::from_secs(2) {
            return Err("scan-timeout");
        }
        result
    }

    async fn exchange(&mut self) -> Result<Capture, &'static str> {
        self.decoder.begin_scan()?;
        let request = format!(
            "{{\"type\":\"scan_request\",\"incarnation\":\"{}\"}}\n",
            self.hello().incarnation
        );
        self.write(request.as_bytes()).await?;
        loop {
            self.read().await?;
            tokio::task::yield_now().await;
            if let Some(capture) = self.decoder.take_capture() {
                if !self.decoder.idle() {
                    return Err("protocol-invalid");
                }
                return Ok(capture);
            }
        }
    }

    async fn write(&self, mut bytes: &[u8]) -> Result<(), &'static str> {
        while !bytes.is_empty() {
            self.stream.writable().await.map_err(|_| "stream-failed")?;
            match self.stream.try_write(bytes) {
                Ok(0) => return Err("stream-failed"),
                Ok(count) => bytes = &bytes[count..],
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => return Err("stream-failed"),
            }
        }
        Ok(())
    }

    async fn read(&mut self) -> Result<(), &'static str> {
        let mut buffer = [0; 4096];
        loop {
            self.stream.readable().await.map_err(|_| "stream-failed")?;
            match self.stream.try_read(&mut buffer) {
                Ok(0) => return Err("stream-eof"),
                Ok(count) => return self.decoder.feed(&buffer[..count]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => return Err("stream-failed"),
            }
        }
    }
}

fn effective_uid() -> Result<u32, &'static str> {
    use std::io::Read;
    // Proc directory ownership can change with dumpability; use the explicit
    // effective-UID field instead. Its line is within this bounded prefix.
    let file = std::fs::File::open("/proc/self/status").map_err(|_| "peer-unverifiable")?;
    let mut bytes = Vec::with_capacity(4096);
    file.take(4096)
        .read_to_end(&mut bytes)
        .map_err(|_| "peer-unverifiable")?;
    let line = bytes
        .split(|byte| *byte == b'\n')
        .find(|line| line.starts_with(b"Uid:"))
        .ok_or("peer-unverifiable")?;
    std::str::from_utf8(line)
        .ok()
        .and_then(|line| line.split_whitespace().nth(2))
        .and_then(|uid| uid.parse().ok())
        .ok_or("peer-unverifiable")
}
