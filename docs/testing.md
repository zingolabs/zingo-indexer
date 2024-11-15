# Testing
### Dependencies
1) [Zebrad](https://github.com/ZcashFoundation/zebra.git)
2) [Lightwalletd](https://github.com/zcash/lightwalletd.git)
4) [Zcashd, Zcash-Cli](https://github.com/zcash/zcash)

### Wallet to Node Tests
- To run tests:
1) Simlink or copy compiled `zcashd`, `zcash-cli`, `zebrad` and `lightwalletd` binaries to `$ zaino/test_binaries/bins/*`
2) Run `$ cargo nextest run --test integrations`

### Client RPC Tests
- To run client rpc tests:
1) Simlink or copy compiled `zebrad`, zcashd` and `zcash-cli` binaries to `$ zaino/test_binaries/bins/*`
2) Build release binary `cargo build --release` WARNING: these tests do not use the binary built by cargo nextest
3) Generate the chain cache `cargo nextest run generate_zcashd_chain_cach --run-ignored ignored-only --features test_fixtures`
4) Run `cargo nextest run --test client_rpcs`

- To Run client rpc testnet tests i.e. `get_subtree_roots_sapling`:
1) sync Zebrad testnet to at least 2 sapling and 2 orchard shards
2) copy the Zebrad cache to `zaino/integration-tests/chain_cache/testnet_get_subtree_roots_sapling` directory.
3) copy the Zebrad cache to `zaino/integration-tests/chain_cache/testnet_get_subtree_roots_orchard` directory.
See the `get_subtree_roots_sapling` test fixture doc comments in zcash_local_net for more details.

