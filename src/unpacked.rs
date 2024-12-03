use std::fs::OpenOptions;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;

use solana_runtime::snapshot_utils::SNAPSHOT_STATUS_CACHE_FILENAME;
use tracing::info;

use crate::append_vec::AppendVec;
use crate::solana::{
    deserialize_from, AccountsDbFields, DeserializableVersionedBank,
    SerializableAccountStorageEntry,
};
use crate::utils::{parse_append_vec_name, ReadProgressTracking};

/// Extracts account data from snapshots that were unarchived to a file system.
pub(crate) struct UnpackedSnapshotExtractor {
    root: PathBuf,
    slot: u64,
    accounts_db_fields: AccountsDbFields<SerializableAccountStorageEntry>,
}

impl UnpackedSnapshotExtractor {
    pub(crate) fn open(path: &Path, progress_tracking: Box<dyn ReadProgressTracking>) -> Self {
        let snapshots_dir = path.join("snapshots");
        let status_cache = snapshots_dir.join(SNAPSHOT_STATUS_CACHE_FILENAME);
        assert!(
            status_cache.is_file(),
            "Status cache is not a file; status_cache={status_cache:?}"
        );

        let snapshot_files = snapshots_dir.read_dir().unwrap();

        let snapshot_file_path = snapshot_files
            .filter_map(|entry| entry.ok())
            .find(|entry| u64::from_str(&entry.file_name().to_string_lossy()).is_ok())
            .map(|entry| entry.path().join(entry.file_name()))
            .unwrap();

        info!("Opening snapshot manifest: {:?}", snapshot_file_path);
        let snapshot_file = OpenOptions::new()
            .read(true)
            .open(&snapshot_file_path)
            .unwrap();
        let snapshot_file_len = snapshot_file.metadata().unwrap().len();

        let snapshot_file = progress_tracking.new_read_progress_tracker(
            &snapshot_file_path,
            Box::new(snapshot_file),
            snapshot_file_len,
        );
        let mut snapshot_file = BufReader::new(snapshot_file);

        let pre_unpack = Instant::now();
        let versioned_bank: DeserializableVersionedBank =
            deserialize_from(&mut snapshot_file).unwrap();
        let slot = versioned_bank.slot;
        drop(versioned_bank);
        let versioned_bank_post_time = Instant::now();

        let accounts_db_fields: AccountsDbFields<SerializableAccountStorageEntry> =
            deserialize_from(&mut snapshot_file).unwrap();
        let accounts_db_fields_post_time = Instant::now();
        drop(snapshot_file);

        info!("Read bank fields in {:?}", versioned_bank_post_time - pre_unpack);
        info!(
            "Read accounts DB fields in {:?}",
            accounts_db_fields_post_time - versioned_bank_post_time
        );

        UnpackedSnapshotExtractor { root: path.to_path_buf(), slot, accounts_db_fields }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) const fn slot(&self) -> u64 {
        self.slot
    }

    pub(crate) fn unboxed_iter(&self) -> impl Iterator<Item = AppendVec> + '_ {
        self.iter_streams()
    }

    fn iter_streams(&self) -> impl Iterator<Item = AppendVec> + '_ {
        let accounts_dir = self.root.join("accounts");
        accounts_dir.read_dir().unwrap().map(move |file| {
            let file = file.unwrap();
            let name = file.file_name();

            let (slot, version) = parse_append_vec_name(&name);

            self.open_append_vec(slot, version, &accounts_dir.join(&name))
        })
    }

    pub(crate) fn open_append_vec(&self, slot: u64, id: u64, path: &Path) -> AppendVec {
        let known_vecs = self
            .accounts_db_fields
            .0
            .get(&slot)
            .map(|v| &v[..])
            .unwrap_or(&[]);
        let known_vec = known_vecs.iter().find(|entry| entry.id == (id as usize));
        let known_vec = match known_vec {
            None => panic!("Unknown vec"),
            Some(v) => v,
        };

        AppendVec::new_from_file(path, known_vec.accounts_current_len, slot, id).unwrap()
    }
}
