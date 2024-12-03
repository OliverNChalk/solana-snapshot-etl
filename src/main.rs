/// Custom implementation of [`solana_accounts_db::append_vec::AppendVec`] with
/// changed visibility & helper methods.
mod append_vec;
mod args;
mod rpc;
mod solana;
mod unpacked;
mod utils;

fn main() {
    use std::sync::mpsc;

    use clap::Parser;
    use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
    use tracing::info;

    use crate::rpc::HistoricalRpc;
    use crate::unpacked::UnpackedSnapshotExtractor;
    use crate::utils::LoadProgressTracking;

    let _ = toolbox::tracing::setup_tracing("solana-snapshot-etl", None);

    let args = args::Args::parse();

    let loader = UnpackedSnapshotExtractor::open(&args.source, Box::new(LoadProgressTracking {}));

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

    // Construct the account index.
    let rpc = HistoricalRpc::load(loader, &accounts_bar, &unique_accounts_bar);
    info!(keys = rpc.account_index.len(), "Accounts index constructed");
    accounts_bar.finish();
    unique_accounts_bar.finish();

    // Bind the RPC server.
    let server = rpc.bind();

    // Register SIGINT handler.
    let (sigint_tx, sigint_rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = sigint_tx.send(());
    })
    .unwrap();

    // Wait for SIGINT & then shutdown the server.
    sigint_rx.recv().unwrap();
    server.close();
}
