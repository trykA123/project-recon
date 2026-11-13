//! Validates bounded external values before they enter RECON's storage operations.

use crate::error::{Error, Result};
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

pub const MAX_INPUT: usize = 65_536;

/// Accepts a nonempty bounded text field without NUL bytes; lengths use UTF-8 bytes.
pub fn text(value: &str, name: &str, max: usize) -> Result<()> {
    if value.trim().is_empty() || value.len() > max || value.contains('\0') {
        return Err(Error::Invalid(format!(
            "{name} must contain 1-{max} UTF-8 bytes without NUL"
        )));
    }
    Ok(())
}

/// Accepts stable ASCII identifiers up to 128 bytes, or returns invalid input.
pub fn id(value: &str) -> Result<()> {
    text(value, "id", 128)?;
    if !value
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
    {
        return Err(Error::Invalid(
            "IDs accept ASCII letters, digits, underscore, dot, and dash".into(),
        ));
    }
    Ok(())
}

/// Validates a capped list of nonempty bounded strings.
pub fn strings(values: &[String], name: &str, count: usize, bytes: usize) -> Result<()> {
    if values.len() > count {
        return Err(Error::Invalid(format!(
            "{name} accepts at most {count} entries"
        )));
    }
    for value in values {
        text(value, name, bytes)?;
    }
    Ok(())
}

/// Converts an estimated-token budget to its explicit serialized UTF-8 byte cap.
pub fn budget(value: usize) -> Result<usize> {
    if !(128..=16_384).contains(&value) {
        return Err(Error::Invalid(
            "budget must be 128-16384 estimated tokens".into(),
        ));
    }
    Ok(value * 4)
}

/// Parses a bounded JSON file, rejecting oversize data and invalid typed fields.
pub fn input<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Invalid("JSON input must be a regular file".into()));
    }
    let mut bytes = Vec::new();
    file.take(u64::try_from(MAX_INPUT + 1).map_err(|_| Error::Budget)?)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_INPUT {
        return Err(Error::Invalid("JSON input exceeds 64 KiB".into()));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

/// Computes a deterministic SHA-256 identifier from unambiguous serialized values.
pub fn digest(value: &impl serde::Serialize) -> Result<String> {
    Ok(hex(&Sha256::digest(serde_json::to_vec(value)?)))
}

/// Encodes bytes as lowercase hexadecimal without an additional runtime dependency.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
