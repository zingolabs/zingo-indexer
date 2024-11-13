# Zaino
A rust implemented indexer and lightwallet service for Zcash.

Zaino is intended to provide all necessary funtionality for clients, including "standalone" (formerly "light") clients/wallets, integrated (formerly "full") client/wallets and block explorers to access both the finalized chain and non-finalized best chain and mempool, held by either a Zebrad or Zcashd full validator.

# Security Vulnerability Disclosure
If you believe you have discovered a security issue, please contact us at:

zingodisclosure@proton.me

# ZainoD
The Zaino Indexer gRPC service.

# Zaino-Serve
Holds a gRPC server capable of servicing clients over both https and the nym mixnet (currently removed due to dependecy conflicts).

Also holds the rust implementations of the LightWallet gRPC Service (CompactTxStreamerServer).

# Zaino-Wallet [*Temporarily Removed due to Nym Dependency Conflict]
Holds the nym-enhanced, wallet-side rust implementations of the LightWallet Service RPCs (NymTxStreamerClient).

* Currently only send_transaction and get_lightd_info are implemented over nym.

# Zaino-State
A mempool and chain-fetching service built on top of zebra's ReadStateService and TrustedChainSync, exosed as a library for direct consumption by full node wallets.

# Zaino-Fetch
A mempool-fetching, chain-fetching and transaction submission service that uses zebra's RPC interface. Used primarily as a backup and legacy option for backwards compatibility.

# Zaino-Nym [*Temporarily Removed due to Nym Dependency Conflict]
Holds backend nym functionality used by Zaino.

# Zaino-Proto
Holds tonic generated code for the lightwallet service RPCs and compact formats.

* We plan to eventually rely on LibRustZcash's versions but hold our our here for development purposes.


# Dependencies
1) zebrad <https://github.com/ZcashFoundation/zebra.git>
2) lightwalletd <https://github.com/zcash/lightwalletd.git> [require for testing]
3) zingolib <https://github.com/zingolabs/zingolib.git> [if running zingo-cli]
4) zcashd, zcash-cli <https://github.com/zcash/zcash> [required until switch to zebras regtest mode]


# Testing
- To run tests:
1) Simlink or copy compiled `zcashd`, `zcash-cli`, `zebrad` and `lightwalletd` binaries to `$ zaino/test_binaries/bins/*`
2) Run `$ cargo nextest run` or `$ cargo test`

- To run client rpc tests:
1) Simlink or copy compiled `zcashd` and `zcash-cli` binaries to `$ zaino/test_binaries/bins/*`
2) Build release binary `cargo build --release` WARNING: these tests do not use the binary built by cargo nextest
3) Generate the chain cache `cargo nextest run generate_zcashd_chain_cach --run-ignored ignored-only --features test_fixtures`
4) Run `cargo nextest run --test client_rpcs`

- To Run client rpc testnet tests i.e. `get_subtree_roots_sapling`:
1) sync Zebrad testnet to at least 2 sapling and 2 orchard shards
2) copy the Zebrad cache to `zaino/integration-tests/chain_cache/testnet_get_subtree_roots_sapling` directory.
3) copy the Zebrad cache to `zaino/integration-tests/chain_cache/testnet_get_subtree_roots_orchard` directory.
See the `get_subtree_roots_sapling` test fixture doc comments in zcash_local_net for more details.

# Running ZainoD
- To run zingo-cli through Zaino, connecting to zebrad locally: [in seperate terminals]
1) Run `$ zebrad --config #PATH_TO_ZINGO_PROXY/zebrad.toml start`
3) Run `$ cargo run`

From #PATH_TO/zingolib:
4) Run `$ cargo run --release --package zingo-cli -- --chain "testnet" --server "127.0.0.1:8080" --data-dir ~/wallets/test_wallet`

# Nym POC [*Temporarily Removed due to Nym Dependency Conflict]
The walletside Nym implementations are moving to ease wallet integration but the POC walletside nym server is still available under the "nym_poc" feature flag.
- To run the POC [in seperate terminals]:
1) Run `$ zebrad --config #PATH_TO_ZINGO_PROXY/zebrad.toml start`
3) Run `$ cargo run`
4) Copy nym address displayed
5) Run `$ cargo run --features "nym_poc" -- <nym address copied>`

From #PATH_TO/zingolib: [send_transaction commands sent with this build will be sent over the mixnet]
6) Run `$ cargo run --release --package zingo-cli -- --chain "testnet" --server "127.0.0.1:8088" --data-dir ~/wallets/testnet_wallet`

Note:
Configuration data can be set using a .toml file (an example zindexer.toml is given in zingo-indexer/zindexer.toml) and can be set at runtime using the --config arg:
- Run `$ cargo run --config zingo-indexerd/zindexer.toml`

