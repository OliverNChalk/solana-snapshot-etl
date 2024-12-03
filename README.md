# Solana Snapshot RPC

Solana Snapshot RPC is built off https://github.com/riptl/solana-snapshot-etl.

## Motivation

Solana nodes periodically backup their account database into a `.tar.zst`
"snapshot" stream. If you run a node yourself, you've probably seen a snapshot
file such as this one already:

```
snapshot-139240745-D17vR2iksG5RoLMfTX7i5NwSsr4VpbybuX1eqzesQfu2.tar.zst
```

A full snapshot file contains a copy of all accounts at a specific slot state (in this case slot `139240745`).

Historical accounts data is relevant to blockchain analytics use-cases and event tracing.
Despite archives being readily available, the ecosystem was missing an easy-to-use tool to access snapshot data.

## Building

```shell
cargo install --git https://github.com/rpcpool/solana-snapshot-rpc
```

## Usage


```txt
$ solana-snapshot-rpc --help
Serve an RPC based on a historical account snapshot

Usage: solana-snapshot-rpc <SOURCE>

Arguments:
  <SOURCE>  Snapshot source (unpacked snapshot)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Source

Serve the RPC from an unpacked snapshot:

```shell
# Unarchive the snapshot.
tar -I zstd -xvf snapshot-*.tar.zst ./unpacked_snapshot/

# Serve the RPC based on the unpacked snapshot state.
solana-snapshot-rpc ./unpacked_snapshot/
```
