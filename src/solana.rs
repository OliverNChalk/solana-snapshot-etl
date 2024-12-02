// Copyright 2022 Solana Foundation.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// This file contains code vendored from https://github.com/solana-labs/solana

use bincode::Options;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use solana_frozen_abi_macro::AbiExample;
use solana_runtime::account_storage::meta::StoredMetaWriteVersion;
use solana_runtime::accounts_db::BankHashStats;
use solana_runtime::ancestors::AncestorsForSerialization;
use solana_runtime::blockhash_queue::BlockhashQueue;
use solana_runtime::epoch_stakes::EpochStakes;
use solana_runtime::rent_collector::RentCollector;
use solana_runtime::stakes::Stakes;
use solana_sdk::clock::{Epoch, UnixTimestamp};
use solana_sdk::deserialize_utils::default_on_eof;
use solana_sdk::epoch_schedule::EpochSchedule;
use solana_sdk::fee_calculator::{FeeCalculator, FeeRateGovernor};
use solana_sdk::hard_forks::HardForks;
use solana_sdk::hash::Hash;
use solana_sdk::inflation::Inflation;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::slot_history::Slot;
use solana_sdk::stake::state::Delegation;
use std::collections::{HashMap, HashSet};
use std::io::Read;

const MAX_STREAM_SIZE: u64 = 32 * 1024 * 1024 * 1024;

pub(crate) fn deserialize_from<R, T>(reader: R) -> bincode::Result<T>
where
    R: Read,
    T: DeserializeOwned,
{
    bincode::options()
        .with_limit(MAX_STREAM_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .deserialize_from::<R, T>(reader)
}

#[derive(Default, PartialEq, Eq, Debug, Deserialize)]
struct UnusedAccounts {
    unused1: HashSet<Pubkey>,
    unused2: HashSet<Pubkey>,
    unused3: HashMap<Pubkey, u64>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct DeserializableVersionedBank {
    pub(crate) blockhash_queue: BlockhashQueue,
    pub(crate) ancestors: AncestorsForSerialization,
    pub(crate) hash: Hash,
    pub(crate) parent_hash: Hash,
    pub(crate) parent_slot: Slot,
    pub(crate) hard_forks: HardForks,
    pub(crate) transaction_count: u64,
    pub(crate) tick_height: u64,
    pub(crate) signature_count: u64,
    pub(crate) capitalization: u64,
    pub(crate) max_tick_height: u64,
    pub(crate) hashes_per_tick: Option<u64>,
    pub(crate) ticks_per_slot: u64,
    pub(crate) ns_per_slot: u128,
    pub(crate) genesis_creation_time: UnixTimestamp,
    pub(crate) slots_per_year: f64,
    pub(crate) accounts_data_len: u64,
    pub(crate) slot: Slot,
    pub(crate) epoch: Epoch,
    pub(crate) block_height: u64,
    pub(crate) collector_id: Pubkey,
    pub(crate) collector_fees: u64,
    pub(crate) fee_calculator: FeeCalculator,
    pub(crate) fee_rate_governor: FeeRateGovernor,
    pub(crate) collected_rent: u64,
    pub(crate) rent_collector: RentCollector,
    pub(crate) epoch_schedule: EpochSchedule,
    pub(crate) inflation: Inflation,
    pub(crate) stakes: Stakes<Delegation>,
    #[allow(dead_code)]
    unused_accounts: UnusedAccounts,
    pub(crate) epoch_stakes: HashMap<Epoch, EpochStakes>,
    pub(crate) is_delta: bool,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq, Eq, AbiExample)]
pub(crate) struct BankHashInfo {
    pub(crate) hash: Hash,
    pub(crate) snapshot_hash: Hash,
    pub(crate) stats: BankHashStats,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct AccountsDbFields<T>(
    pub(crate) HashMap<Slot, Vec<T>>,
    pub(crate) StoredMetaWriteVersion,
    pub(crate) Slot,
    pub(crate) BankHashInfo,
    /// all slots that were roots within the last epoch
    #[serde(deserialize_with = "default_on_eof")]
    pub(crate) Vec<Slot>,
    /// slots that were roots within the last epoch for which we care about the hash value
    #[serde(deserialize_with = "default_on_eof")]
    pub(crate) Vec<(Slot, Hash)>,
);

pub(crate) type SerializedAppendVecId = usize;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize)]
pub(crate) struct SerializableAccountStorageEntry {
    pub(crate) id: SerializedAppendVecId,
    pub(crate) accounts_current_len: usize,
}
