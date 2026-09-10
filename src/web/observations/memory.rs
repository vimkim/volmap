//! Conservative allocation reservations travel with retained objects.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub(super) const MIB: usize = 1024 * 1024;
#[derive(Clone)]
pub(super) struct Budget(Arc<AtomicUsize>);
impl Default for Budget {
    fn default() -> Self {
        Self(Arc::new(AtomicUsize::new(0)))
    }
}
impl Budget {
    pub fn reserve(&self, bytes: usize) -> Result<Charge, &'static str> {
        self.0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes).filter(|total| *total <= 128 * MIB)
            })
            .map_err(|_| "runtime-memory-admission")?;
        Ok(Charge {
            budget: self.clone(),
            bytes,
        })
    }
}
pub(super) struct Charge {
    budget: Budget,
    bytes: usize,
}
impl std::fmt::Debug for Charge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Charge").field(&self.bytes).finish()
    }
}
impl Drop for Charge {
    fn drop(&mut self) {
        self.budget.0.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

pub(in crate::web) struct ResponseBytes {
    pub bytes: Vec<u8>,
    pub(super) _charge: Charge,
    pub(super) _permit: tokio::sync::OwnedSemaphorePermit,
}
impl AsRef<[u8]> for ResponseBytes {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_boundary_counts_retained_inflight_and_response_owners_until_last_drop() {
        let budget = Budget::default();
        let session = budget.reserve(8 * MIB).unwrap();
        let parser = budget.reserve(16 * MIB).unwrap();
        let old = Arc::new(budget.reserve(32 * MIB).unwrap());
        let retained = old.clone();
        let inflight = budget.reserve(32 * MIB).unwrap();
        let responses: Vec<_> = (0..8).map(|_| budget.reserve(2 * MIB).unwrap()).collect();
        let below = budget.reserve(24 * MIB - 1).unwrap();
        let at = budget.reserve(1).unwrap();
        assert!(budget.reserve(1).is_err());
        drop(old);
        assert!(
            budget.reserve(1).is_err(),
            "replacement cannot uncharge a surviving reference"
        );
        drop(retained);
        let replacement = budget.reserve(32 * MIB).unwrap();
        assert!(budget.reserve(1).is_err());
        drop((session, parser, inflight, responses, below, at, replacement));
        assert!(budget.reserve(128 * MIB).is_ok());
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    use crate::web::observations::wire::Decoder;

    #[test]
    fn actual_decoders_cannot_admit_object_caps_that_exceed_the_shared_total() {
        let budget = Budget::default();
        let _session = budget.reserve(8 * MIB).unwrap();
        let responses: Vec<_> = (0..8).map(|_| budget.reserve(2 * MIB).unwrap()).collect();
        let transcript = include_str!(
            "../../../fixtures/pgbuf-inspector/v1/corpus/exchanges/complete/stream.jsonl"
        );
        let hello = format!("{}\n", transcript.lines().nth(1).unwrap());
        let header = format!("{}\n", transcript.lines().nth(3).unwrap());
        let capture = || {
            let mut decoder = Decoder::with_budget(budget.clone()).unwrap();
            decoder.feed(hello.as_bytes()).unwrap();
            decoder.begin_scan().unwrap();
            for line in transcript.lines().skip(3) {
                decoder.feed(format!("{line}\n").as_bytes()).unwrap();
            }
            decoder.take_capture().unwrap()
        };
        let old = capture();
        let retained = capture();
        let mut inflight = Decoder::with_budget(budget.clone()).unwrap();
        inflight.feed(hello.as_bytes()).unwrap();
        inflight.begin_scan().unwrap();
        // 8 session + 16 responses + 32 old + 32 latest + 16 parser = 104 MiB.
        // A 32 MiB capture remains individually legal but cannot fit the total.
        assert_eq!(
            inflight.feed(header.as_bytes()),
            Err("runtime-memory-admission")
        );
        drop(old);
        let mut replacement = Decoder::with_budget(budget.clone()).unwrap();
        replacement.feed(hello.as_bytes()).unwrap();
        replacement.begin_scan().unwrap();
        replacement.feed(header.as_bytes()).unwrap();
        // 120 MiB including the failed decoder's still-owned parser scratch.
        let below = budget.reserve(8 * MIB - 1).unwrap();
        let at = budget.reserve(1).unwrap();
        assert!(budget.reserve(1).is_err());
        drop((retained, responses, replacement, inflight, below, at));
        assert!(budget.reserve(120 * MIB).is_ok());
    }
}
