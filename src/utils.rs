use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;

use thiserror::Error;

use crate::append_vec::{AppendVec, StoredAccountMeta};

#[derive(Error, Debug)]
pub(crate) enum SnapshotError {
    #[error("{0}")]
    IOError(#[from] std::io::Error),
    #[error("Failed to deserialize: {0}")]
    BincodeError(#[from] bincode::Error),
    #[error("Missing status cache")]
    NoStatusCache,
    #[error("No snapshot manifest file found")]
    NoSnapshotManifest,
    #[error("Unexpected AppendVec")]
    UnexpectedAppendVec,
    #[error("Failed to create read progress tracking: {0}")]
    ReadProgressTracking(String),
}

pub(crate) type SnapshotResult<T> = Result<T, SnapshotError>;

pub(crate) fn parse_append_vec_name(name: &OsStr) -> Option<(u64, u64)> {
    let name = name.to_str()?;
    let mut parts = name.splitn(2, '.');
    let slot = u64::from_str(parts.next().unwrap_or(""));
    let id = u64::from_str(parts.next().unwrap_or(""));
    match (slot, id) {
        (Ok(slot), Ok(version)) => Some((slot, version)),
        _ => {
            println!("PARSE FAIL: {name:?}");
            None
        }
    }
}

pub(crate) fn append_vec_iter(
    append_vec: &AppendVec,
) -> impl Iterator<Item = StoredAccountMetaHandle> {
    let mut offset = 0usize;
    std::iter::repeat_with(move || {
        append_vec.get_account(offset).map(|(_, next_offset)| {
            let account = StoredAccountMetaHandle::new(append_vec, offset);
            offset = next_offset;
            account
        })
    })
    .take_while(|account| account.is_some())
    .flatten()
}

pub(crate) struct StoredAccountMetaHandle<'a> {
    append_vec: &'a AppendVec,
    offset: usize,
}

impl<'a> StoredAccountMetaHandle<'a> {
    pub(crate) const fn new(
        append_vec: &'a AppendVec,
        offset: usize,
    ) -> StoredAccountMetaHandle<'a> {
        Self { append_vec, offset }
    }

    pub(crate) fn access(&self) -> Option<StoredAccountMeta<'_>> {
        Some(self.append_vec.get_account(self.offset)?.0)
    }
}

pub(crate) trait ReadProgressTracking {
    fn new_read_progress_tracker(
        &self,
        path: &Path,
        rd: Box<dyn Read>,
        file_len: u64,
    ) -> SnapshotResult<Box<dyn Read>>;
}
