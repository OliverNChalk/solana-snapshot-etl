use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub(crate) struct Args {
    /// Snapshot source (unpacked snapshot).
    #[clap(long)]
    pub(crate) source: PathBuf,
}
