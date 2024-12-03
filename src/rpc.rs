use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;

use hashbrown::HashMap;
use indicatif::ProgressBar;
use jsonrpc_core::{BoxFuture, Error as JsonRpcError, MetaIoHandler, Result};
use jsonrpc_derive::rpc;
use jsonrpc_http_server::{
    hyper, AccessControlAllowOrigin, DomainsValidation, Server, ServerBuilder,
};
use solana_account_decoder::{encode_ui_account, UiAccount, UiAccountEncoding};
use solana_rpc::rpc::verify_pubkey;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::config::{
    RpcAccountInfoConfig, RpcEncodingConfigWrapper, RpcTransactionConfig,
};
use solana_rpc_client_api::response::{Response as RpcResponse, RpcResponseContext};
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_transaction_status::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use tracing::{debug, info};

use crate::unpacked::UnpackedSnapshotExtractor;
use crate::utils::append_vec_iter;

const EXPECTED_ACCOUNTS: usize = 800_000_000;
const LISTEN_ADDRESS: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 8899));

pub(crate) struct HistoricalRpc {
    extractor: UnpackedSnapshotExtractor,
    account_index: HashMap<Pubkey, (u64, u64)>,
    transaction_rpc: Option<RpcClient>,
}

impl HistoricalRpc {
    pub(crate) fn load(
        extractor: UnpackedSnapshotExtractor,
        accounts_bar: &ProgressBar,
        unique_accounts_bar: &ProgressBar,
        transaction_rpc: Option<String>,
    ) -> Self {
        let transaction_rpc = transaction_rpc.map(|url| RpcClient::new(url));

        let mut account_index = HashMap::with_capacity(EXPECTED_ACCOUNTS);
        for append_vec in extractor.unboxed_iter().take(10) {
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

        info!(keys = account_index.len(), "Accounts index constructed");
        accounts_bar.finish();
        unique_accounts_bar.finish();

        HistoricalRpc { extractor, account_index, transaction_rpc }
    }

    pub(crate) const fn slot(&self) -> u64 {
        self.extractor.slot()
    }

    pub(crate) fn get_account(&self, key: &Pubkey) -> Option<Account> {
        let (slot, id) = *self.account_index.get(key)?;

        let path = self.extractor.root().join(format!("accounts/{slot}.{id}"));
        let vec = self.extractor.open_append_vec(slot, id, &path);
        let account = append_vec_iter(&vec)
            .find(|account| &account.access().unwrap().meta.pubkey == key)
            .unwrap()
            .access()
            .unwrap()
            .clone_account();

        Some(account)
    }

    async fn get_transaction(
        &self,
        signature: Signature,
        config: Option<RpcEncodingConfigWrapper<RpcTransactionConfig>>,
    ) -> Result<Option<EncodedConfirmedTransactionWithStatusMeta>> {
        let Some(rpc) = &self.transaction_rpc else {
            return Err(JsonRpcError::invalid_params(format!(
                "This historical RPC does not have a provided transaction_rpc"
            )));
        };

        let config = config
            .and_then(|config| match config {
                RpcEncodingConfigWrapper::Current(config) => config,
                RpcEncodingConfigWrapper::Deprecated(_) => None,
            })
            .unwrap_or_else(|| RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::Base64),
                max_supported_transaction_version: Some(1),
                commitment: None,
            });

        rpc.get_transaction_with_config(&signature, config)
            .await
            .map(|tx| Some(tx))
            .map_err(|err| {
                JsonRpcError::invalid_params(format!("transaction_rpc failed; err={err:?}"))
            })
    }

    pub(crate) fn bind(self) -> Server {
        let historical_rpc = Arc::new(self);

        // Bind the RPC server.
        let mut io = MetaIoHandler::default();
        io.extend_with(AccountsRpcImpl.to_delegate());

        ServerBuilder::with_meta_extractor(io, move |_: &hyper::Request<hyper::Body>| {
            historical_rpc.clone()
        })
        .threads(1)
        .cors(DomainsValidation::AllowOnly(vec![AccessControlAllowOrigin::Any]))
        .cors_max_age(86400)
        .start_http(&LISTEN_ADDRESS)
        .unwrap()
    }
}

#[rpc]
pub trait AccountsRpc {
    type Metadata;

    #[rpc(meta, name = "getAccountInfo")]
    fn get_account_info(
        &self,
        meta: Self::Metadata,
        pubkey_str: String,
        config: Option<RpcAccountInfoConfig>,
    ) -> Result<RpcResponse<Option<UiAccount>>>;

    #[rpc(meta, name = "getTransaction")]
    fn get_transaction(
        &self,
        meta: Self::Metadata,
        signature_str: String,
        config: Option<RpcEncodingConfigWrapper<RpcTransactionConfig>>,
    ) -> BoxFuture<Result<Option<EncodedConfirmedTransactionWithStatusMeta>>>;
}

struct AccountsRpcImpl;

impl AccountsRpc for AccountsRpcImpl {
    type Metadata = Arc<HistoricalRpc>;

    fn get_account_info(
        &self,
        meta: Self::Metadata,
        pubkey: String,
        config: Option<RpcAccountInfoConfig>,
    ) -> Result<RpcResponse<Option<UiAccount>>> {
        debug!(pubkey, "get_account_info rpc request received");
        let pubkey = verify_pubkey(&pubkey)?;
        let slot = meta.slot();

        // Validate arguments.
        let RpcAccountInfoConfig { encoding, data_slice, min_context_slot, .. } =
            config.unwrap_or_default();
        let min_context_slot = min_context_slot.unwrap_or(0);
        if encoding != Some(UiAccountEncoding::Base64) {
            return Err(JsonRpcError::invalid_params(format!(
                "Expected base64 encoding; received={encoding:?}"
            )));
        }
        if data_slice.is_some() {
            return Err(JsonRpcError::invalid_params(format!(
                "Account data_slice unsupported; received={data_slice:?}"
            )));
        }
        if min_context_slot > meta.slot() {
            return Err(JsonRpcError::invalid_params(format!(
                "Min context slot not reached; requested={min_context_slot}; highest={slot}",
            )));
        }

        // Load the account.
        let account = meta.get_account(&pubkey).map(|account| {
            encode_ui_account(&pubkey, &account, UiAccountEncoding::Base64, None, None)
        });

        Ok(RpcResponse { context: RpcResponseContext::new(slot), value: account })
    }

    fn get_transaction(
        &self,
        meta: Self::Metadata,
        signature_str: String,
        config: Option<RpcEncodingConfigWrapper<RpcTransactionConfig>>,
    ) -> BoxFuture<Result<Option<EncodedConfirmedTransactionWithStatusMeta>>> {
        let signature = signature_str
            .parse()
            .map_err(|e| JsonRpcError::invalid_params(format!("Invalid param: {e:?}")));
        match signature {
            Ok(signature) => Box::pin(async move { meta.get_transaction(signature, config).await }),
            Err(err) => Box::pin(futures::future::err(err)),
        }
    }
}
