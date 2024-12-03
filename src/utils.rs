use std::ffi::OsStr;
use std::io::{IoSliceMut, Read};
use std::path::Path;
use std::str::FromStr;

use indicatif::{ProgressBar, ProgressBarIter, ProgressStyle};

use crate::append_vec::{AppendVec, StoredAccountMeta};

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
    ) -> Box<dyn Read>;
}

pub(crate) struct LoadProgressTracking {}

impl ReadProgressTracking for LoadProgressTracking {
    fn new_read_progress_tracker(
        &self,
        _path: &Path,
        rd: Box<dyn Read>,
        file_len: u64,
    ) -> Box<dyn Read> {
        let progress_bar = ProgressBar::new(file_len).with_style(
            ProgressStyle::with_template(
                "{prefix:>15.bold.dim} {spinner:.green} [{bar:.cyan/blue}] {bytes}/{total_bytes} \
                 ({percent}%)",
            )
            .unwrap()
            .progress_chars("#>-"),
        );
        progress_bar.set_prefix("manifest");

        Box::new(LoadProgressTracker { rd: progress_bar.wrap_read(rd), progress_bar })
    }
}

struct LoadProgressTracker {
    progress_bar: ProgressBar,
    rd: ProgressBarIter<Box<dyn Read>>,
}

impl Drop for LoadProgressTracker {
    fn drop(&mut self) {
        self.progress_bar.finish()
    }
}

impl Read for LoadProgressTracker {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.rd.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> std::io::Result<usize> {
        self.rd.read_vectored(bufs)
    }

    fn read_to_string(&mut self, buf: &mut String) -> std::io::Result<usize> {
        self.rd.read_to_string(buf)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        self.rd.read_exact(buf)
    }
}
