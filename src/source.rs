//! Captures and verifies bounded evidence files under a project's registered root.

use crate::{
    error::{Error, Result},
    model::EvidenceInput,
};
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir, OpenOptions};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Component, Path},
};

const MAX_SOURCE: u64 = 16 * 1024 * 1024;

/// Hashes one contained relative source file; rejects traversal, escapes, and oversized files.
pub fn hash(root: &Path, relative: &str) -> Result<String> {
    let path = Path::new(relative);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(Error::Invalid(
            "evidence paths must be relative without traversal".into(),
        ));
    }
    let mut file = open(root, path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Invalid(
            "evidence must reference a regular file".into(),
        ));
    }
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 8192];
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total += u64::try_from(read).map_err(|_| Error::Budget)?;
        if total > MAX_SOURCE {
            return Err(Error::Invalid("evidence source exceeds 16 MiB".into()));
        }
        digest.update(&buffer[..read]);
    }
    Ok(crate::validate::hex(&digest.finalize()))
}

/// Opens a regular file through a confined directory capability, without blocking on Unix FIFOs.
fn open(root: &Path, relative: &Path) -> Result<cap_std::fs::File> {
    let directory = Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    let metadata = directory.metadata(relative).map_err(containment_error)?;
    if !metadata.is_file() {
        return Err(Error::Invalid(
            "evidence must reference a regular file".into(),
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NONBLOCK);
    directory
        .open_with(relative, &options)
        .map_err(containment_error)
}

/// Reports denied confined paths as invalid input while preserving other filesystem failures.
fn containment_error(error: std::io::Error) -> Error {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        Error::Invalid("evidence path is inaccessible or escapes the registered root".into())
    } else {
        Error::Io(error)
    }
}

/// Captures source hashes, rejecting evidence supplied for different current bytes.
pub fn capture(root: &Path, sources: &[EvidenceInput]) -> Result<Vec<String>> {
    sources
        .iter()
        .map(|source| {
            let current = hash(root, &source.path)?;
            if source
                .source_hash
                .as_ref()
                .is_some_and(|expected| !expected.eq_ignore_ascii_case(&current))
            {
                return Err(Error::SourceChanged(source.path.clone()));
            }
            Ok(current)
        })
        .collect()
}
