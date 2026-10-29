//! Append-only, hash-chained storage.
//!
//! Every record commits to the hash of the record before it, so altering,
//! removing or reordering any past record breaks the chain and `verify`
//! reports exactly where. The `Storage` trait is the seam that lets the
//! in-memory store be swapped for Postgres now and a custom engine later.

use sha2::{Digest, Sha256};
use thiserror::Error;

pub type Hash = [u8; 32];

/// Hash that the first record chains from.
pub const GENESIS: Hash = [0u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub seq: u64,
    pub prev_hash: Hash,
    pub hash: Hash,
    pub payload: Vec<u8>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChainError {
    #[error("record {seq}: sequence out of order (expected {expected})")]
    Sequence { seq: u64, expected: u64 },
    #[error("record {seq}: does not link to the previous record")]
    BrokenLink { seq: u64 },
    #[error("record {seq}: contents do not match its hash (tampered)")]
    Tampered { seq: u64 },
}

/// Anything that can durably hold the event log.
pub trait Storage {
    /// Append a payload and return the sealed record.
    fn append(&mut self, payload: Vec<u8>) -> Record;
    /// All records in commit order.
    fn records(&self) -> &[Record];
    /// Hash of the latest record, the value to anchor externally.
    fn head(&self) -> Hash {
        self.records().last().map_or(GENESIS, |r| r.hash)
    }
    /// Recompute the whole chain and report the first fault.
    fn verify(&self) -> Result<(), ChainError> {
        let mut prev = GENESIS;
        for (i, r) in self.records().iter().enumerate() {
            let expected = i as u64;
            if r.seq != expected {
                return Err(ChainError::Sequence { seq: r.seq, expected });
            }
            if r.prev_hash != prev {
                return Err(ChainError::BrokenLink { seq: r.seq });
            }
            if seal(r.seq, &r.prev_hash, &r.payload) != r.hash {
                return Err(ChainError::Tampered { seq: r.seq });
            }
            prev = r.hash;
        }
        Ok(())
    }
}

/// sha256(seq || prev_hash || payload)
pub fn seal(seq: u64, prev_hash: &Hash, payload: &[u8]) -> Hash {
    let mut h = Sha256::new();
    h.update(seq.to_be_bytes());
    h.update(prev_hash);
    h.update(payload);
    h.finalize().into()
}

/// In-memory implementation; the reference for every other backend.
#[derive(Debug, Default)]
pub struct MemoryStore {
    records: Vec<Record>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Test hook: mutable access so tamper detection can be exercised.
    #[doc(hidden)]
    pub fn records_mut(&mut self) -> &mut Vec<Record> {
        &mut self.records
    }
}

impl Storage for MemoryStore {
    fn append(&mut self, payload: Vec<u8>) -> Record {
        let seq = self.records.len() as u64;
        let prev_hash = self.head();
        let hash = seal(seq, &prev_hash, &payload);
        let record = Record { seq, prev_hash, hash, payload };
        self.records.push(record.clone());
        record
    }

    fn records(&self) -> &[Record] {
        &self.records
    }
}

/// Lowercase hex, for display and anchoring.
pub fn hex(hash: &Hash) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with(n: usize) -> MemoryStore {
        let mut s = MemoryStore::new();
        for i in 0..n {
            s.append(format!("event-{i}").into_bytes());
        }
        s
    }

    #[test]
    fn chain_links_and_verifies() {
        let s = store_with(5);
        assert_eq!(s.records()[0].prev_hash, GENESIS);
        assert_eq!(s.records()[3].prev_hash, s.records()[2].hash);
        assert_eq!(s.verify(), Ok(()));
    }

    #[test]
    fn edited_payload_is_detected() {
        let mut s = store_with(5);
        s.records_mut()[2].payload = b"forged".to_vec();
        assert_eq!(s.verify(), Err(ChainError::Tampered { seq: 2 }));
    }

    #[test]
    fn deleted_record_is_detected() {
        let mut s = store_with(5);
        s.records_mut().remove(1);
        assert_eq!(s.verify(), Err(ChainError::Sequence { seq: 2, expected: 1 }));
    }

    #[test]
    fn resealed_forgery_breaks_the_next_link() {
        let mut s = store_with(5);
        let r = &mut s.records_mut()[2];
        r.payload = b"forged".to_vec();
        r.hash = seal(r.seq, &r.prev_hash, &r.payload);
        assert_eq!(s.verify(), Err(ChainError::BrokenLink { seq: 3 }));
    }
}
