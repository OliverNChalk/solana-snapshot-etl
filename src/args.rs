use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub(crate) struct Args {
    /// Snapshot source (unpacked snapshot).
    pub(crate) source: PathBuf,
    /// Requests to `getTransaction` will be forward to this RPC.
    #[clap(long)]
    pub(crate) transaction_rpc: Option<String>,
}
