use hashbrown::HashMap;

use indicatif::ProgressBar;
use solana_sdk::{account::Account, pubkey::Pubkey};

use crate::{unpacked::UnpackedSnapshotExtractor, utils::append_vec_iter};

const EXPECTED_ACCOUNTS: usize = 10_000;

pub(crate) struct HistoricalRpc {
    extractor: UnpackedSnapshotExtractor,
    pub(crate) account_index: HashMap<Pubkey, (u64, u64)>,
}

impl HistoricalRpc {
    pub(crate) fn load(
        extractor: UnpackedSnapshotExtractor,
        accounts_bar: &ProgressBar,
        unique_accounts_bar: &ProgressBar,
    ) -> Self {
        let mut account_index = HashMap::with_capacity(EXPECTED_ACCOUNTS);
        for append_vec in extractor.unboxed_iter().map(|vec| vec.unwrap()).take(10) {
            let slot = append_vec.slot();
            let id = append_vec.id();

            for account in append_vec_iter(&append_vec).take(2) {
                accounts_bar.inc(1);

                let account = account.access().unwrap();
                let key = account.meta.pubkey;
                println!("{key}");

                // Insert the slot if it's newer.
                let entry = account_index.entry(key).or_insert_with(|| {
                    unique_accounts_bar.inc(1);

                    (slot, id)
                });
                if entry.0 < slot {
                    *entry = (slot, id);
                }
            }
        }

        HistoricalRpc {
            extractor,
            account_index,
        }
    }

    pub(crate) fn get_account(&self, key: &Pubkey) -> Option<Account> {
        let (slot, id) = *self.account_index.get(key)?;

        let path = self.extractor.root().join(format!("accounts/{slot}.{id}"));
        let vec = self.extractor.open_append_vec(slot, id, &path).unwrap();
        let account = append_vec_iter(&vec)
            .find(|account| &account.access().unwrap().meta.pubkey == key)
            .unwrap()
            .access()
            .unwrap()
            .clone_account();

        Some(account)
    }
}
