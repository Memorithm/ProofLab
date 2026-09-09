//! Canonical encoding and hashing for content-addressed `ProofLab` artifacts.
//!
//! Adapted from `Memorithm/SciRust@267af914bd4a72af531c3d22afd222dd62bd3a70`,
//! `sos/sos-core/src/canonical.rs`. The `ProofLab` variant keeps the same core
//! invariants: explicit type tags, fixed-width integers, length-prefixed byte
//! strings and deterministic sequence order.

use sha2::{Digest, Sha256};

const T_U64: u8 = 0x01;
const T_I64: u8 = 0x02;
const T_BOOL: u8 = 0x03;
const T_BYTES: u8 = 0x04;
const T_STR: u8 = 0x05;
const T_SEQ: u8 = 0x06;
const T_SOME: u8 = 0x07;
const T_NONE: u8 = 0x08;

/// A deterministic, self-delimiting byte encoder for content identity.
#[derive(Debug, Clone, Default)]
pub struct CanonicalEncoder {
    buf: Vec<u8>,
}

impl CanonicalEncoder {
    /// Create an empty encoder.
    #[must_use]
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Consume the encoder and return its canonical bytes.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }

    fn tag(&mut self, tag: u8) {
        self.buf.push(tag);
    }

    fn push_len(&mut self, len: usize) {
        self.buf.extend_from_slice(&(len as u64).to_le_bytes());
    }

    /// Encode an unsigned 64-bit integer.
    pub fn u64(&mut self, value: u64) {
        self.tag(T_U64);
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Encode a signed 64-bit integer.
    pub fn i64(&mut self, value: i64) {
        self.tag(T_I64);
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// Encode a boolean.
    pub fn bool(&mut self, value: bool) {
        self.tag(T_BOOL);
        self.buf.push(u8::from(value));
    }

    /// Encode an opaque byte string.
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.tag(T_BYTES);
        self.push_len(bytes.len());
        self.buf.extend_from_slice(bytes);
    }

    /// Encode a UTF-8 string.
    pub fn str(&mut self, value: &str) {
        self.tag(T_STR);
        self.push_len(value.len());
        self.buf.extend_from_slice(value.as_bytes());
    }

    /// Encode a nested canonical value.
    pub fn value<T: Canonical + ?Sized>(&mut self, value: &T) {
        value.encode(self);
    }

    /// Encode an ordered homogeneous sequence.
    pub fn seq<T: Canonical>(&mut self, items: &[T]) {
        self.tag(T_SEQ);
        self.push_len(items.len());
        for item in items {
            item.encode(self);
        }
    }

    /// Encode an optional value.
    pub fn option<T: Canonical>(&mut self, value: &Option<T>) {
        match value {
            Some(inner) => {
                self.tag(T_SOME);
                inner.encode(self);
            }
            None => self.tag(T_NONE),
        }
    }
}

/// A value with a deterministic canonical byte representation.
pub trait Canonical {
    /// Append the value to `encoder` in canonical form.
    fn encode(&self, encoder: &mut CanonicalEncoder);

    /// Encode into a fresh byte buffer.
    #[must_use]
    fn canonical_bytes(&self) -> Vec<u8> {
        let mut encoder = CanonicalEncoder::new();
        self.encode(&mut encoder);
        encoder.finish()
    }
}

/// Compute a domain-separated SHA-256 digest of a canonical value.
#[must_use]
pub fn sha256_canonical<T: Canonical + ?Sized>(domain: &[u8], value: &T) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(value.canonical_bytes());
    hash.finalize().into()
}

/// Compute a SHA-256 digest over exact bytes.
#[must_use]
pub fn sha256_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

impl Canonical for u64 {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.u64(*self);
    }
}

impl Canonical for u8 {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.u64(u64::from(*self));
    }
}

impl Canonical for i64 {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.i64(*self);
    }
}

impl Canonical for i32 {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.i64(i64::from(*self));
    }
}

impl Canonical for bool {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bool(*self);
    }
}

impl Canonical for str {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str(self);
    }
}

impl Canonical for String {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.str(self);
    }
}

impl Canonical for [u8; 32] {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.bytes(self);
    }
}

impl<T: Canonical> Canonical for Vec<T> {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.seq(self);
    }
}

impl<T: Canonical> Canonical for Option<T> {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        encoder.option(self);
    }
}

impl<T: Canonical + ?Sized> Canonical for &T {
    fn encode(&self, encoder: &mut CanonicalEncoder) {
        (*self).encode(encoder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_encoding_is_deterministic_and_type_tagged() {
        assert_eq!(
            "proof".canonical_bytes(),
            String::from("proof").canonical_bytes()
        );
        assert_ne!(1u64.canonical_bytes(), String::from("1").canonical_bytes());
        assert_ne!(
            vec![String::from("a"), String::from("b")].canonical_bytes(),
            vec![String::from("ab")].canonical_bytes()
        );
    }

    #[test]
    fn domain_separation_changes_identity() {
        assert_ne!(
            sha256_canonical(b"prooflab:a\0", &7u64),
            sha256_canonical(b"prooflab:b\0", &7u64)
        );
    }
}
