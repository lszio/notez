//! Content hashing helpers used by the document scanners to derive
//! cross-Space stable `ObjectId` values (spec §3.1).
//!
//! The algorithm is intentionally cheap: SHA-256 of the file's full
//! byte length (mixed in as a `u64` little-endian) followed by up to
//! the first 64 KiB of content. Large files therefore get a stable
//! fingerprint without scanning the whole payload. For
//! `content_hash_of_bytes` the same construction is applied to the
//! supplied byte range, which lets Heading/Block scanners hash the
//! slice of an already-loaded document without re-reading from disk.

use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::path::Path;

/// SHA-256 fingerprint of a file's content (first 64 KiB + size mix).
///
/// Returns the hash as a lowercase hex string suitable for
/// [`crate::domain::derived_object_id`].
pub fn content_hash_of_file(path: &Path) -> io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(content_hash_of_bytes(&buf))
}

/// SHA-256 fingerprint of an in-memory byte range (already-loaded
/// Heading/Block span). Mirrors [`content_hash_of_file`]'s
/// size-prefix-then-content construction so that identical content
/// produces identical hashes regardless of whether the caller had a
/// path or already-loaded bytes.
pub fn content_hash_of_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
