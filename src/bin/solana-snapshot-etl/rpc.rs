use hashbrown::HashMap;

use indicatif::ProgressBar;
use solana_sdk::pubkey::Pubkey;
use solana_snapshot_etl::{append_vec_iter, SnapshotExtractor};

use crate::SupportedLoader;

pub(crate) struct HistoricalRpc {
    pub(crate) account_index: HashMap<Pubkey, u64>,
}

impl HistoricalRpc {
    pub(crate) fn load(
        mut loader: SupportedLoader,
        accounts_bar: &ProgressBar,
        unique_accounts_bar: &ProgressBar,
    ) -> Self {
        let mut rpc = HistoricalRpc {
            account_index: HashMap::with_capacity(750_000_000),
        };
        for append_vec in loader.iter().map(|vec| vec.unwrap()) {
            let slot = append_vec.slot();

            for account in append_vec_iter(&append_vec) {
                accounts_bar.inc(1);

                let account = account.access().unwrap();
                let key = account.meta.pubkey;

                // Insert the slot if it's newer.
                let entry = rpc.account_index.entry(key).or_insert_with(|| {
                    unique_accounts_bar.inc(1);

                    slot
                });
                if *entry < slot {
                    *entry = slot;
                }
            }
        }

        rpc
    }
}
