use std::io::{IoSliceMut, Read};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use indicatif::{MultiProgress, ProgressBar, ProgressBarIter, ProgressStyle};
use rpc::HistoricalRpc;
use solana_sdk::pubkey::Pubkey;
use tracing::info;
use unpacked::UnpackedSnapshotExtractor;
use utils::{ReadProgressTracking, SnapshotError, SnapshotResult};

mod append_vec;
mod rpc;
mod solana;
mod unpacked;
mod utils;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Args {
    /// Snapshot source (unpacked snapshot).
    #[clap(long)]
    source: PathBuf,

    /// Number of threads used to process snapshot, by default number of CPUs
    /// would be used.
    #[clap(long)]
    num_threads: Option<usize>,

    #[command(subcommand)]
    action: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Index all accounts and serve an RPC.
    Rpc,
}

fn main() {
    let _ = toolbox::tracing::setup_tracing("solana-snapshot-etl", None);

    let args = Args::parse();

    let loader =
        UnpackedSnapshotExtractor::open(&args.source, Box::new(LoadProgressTracking {})).unwrap();
    // SupportedLoader::new(&args.source, Box::new(LoadProgressTracking
    // {})).unwrap();

    // Setup a multi progress bar & style.
    let multi = MultiProgress::new();
    let style = ProgressStyle::with_template(
        "{prefix:>15.bold.dim} {spinner:.green} rate={per_sec} processed={human_pos} \
         {elapsed_precise:.cyan}",
    )
    .unwrap();

    // Setup accounts processed bar.
    let accounts_bar = multi.add(ProgressBar::new_spinner());
    accounts_bar.set_prefix("accounts");
    accounts_bar.set_style(style.clone());

    // Setup unique accounts processed bar.
    let unique_accounts_bar = multi.add(ProgressBar::new_spinner());
    unique_accounts_bar.set_prefix("unique accounts");
    unique_accounts_bar.set_style(style);

    match args.action {
        Action::Rpc => {
            // Construct the account index.
            let rpc = HistoricalRpc::load(loader, &accounts_bar, &unique_accounts_bar);

            info!(keys = rpc.account_index.len(), "Accounts index constructed");
            accounts_bar.finish();
            unique_accounts_bar.finish();

            // Request input from user for which historical account to lookup.
            let mut request_buf = String::new();
            loop {
                request_buf.clear();

                println!("\nPlease enter the account you want to load: ");
                std::io::stdin().read_line(&mut request_buf).unwrap();

                // Find account slot.
                let key = match request_buf.trim().parse::<Pubkey>() {
                    Ok(key) => key,
                    Err(err) => {
                        println!("INVALID KEY: key={request_buf}; err={err}");
                        continue;
                    }
                };
                let (slot, id) = match rpc.account_index.get(&key) {
                    Some(slot) => slot,
                    None => {
                        println!("MISSING; key={key}");
                        continue;
                    }
                };

                // Lookup the slot.
                println!("Found; slot={slot}; id={id}");
                let account = rpc.get_account(&key).unwrap();
                println!("{account:?}");
            }
        }
    }
}

struct LoadProgressTracking {}

impl ReadProgressTracking for LoadProgressTracking {
    fn new_read_progress_tracker(
        &self,
        _path: &Path,
        rd: Box<dyn Read>,
        file_len: u64,
    ) -> SnapshotResult<Box<dyn Read>> {
        let progress_bar = ProgressBar::new(file_len).with_style(
            ProgressStyle::with_template(
                "{prefix:>15.bold.dim} {spinner:.green} [{bar:.cyan/blue}] {bytes}/{total_bytes} \
                 ({percent}%)",
            )
            .map_err(|error| SnapshotError::ReadProgressTracking(error.to_string()))?
            .progress_chars("#>-"),
        );
        progress_bar.set_prefix("manifest");
        Ok(Box::new(LoadProgressTracker { rd: progress_bar.wrap_read(rd), progress_bar }))
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
