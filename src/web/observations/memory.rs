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
