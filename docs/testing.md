# Testing
### Dependencies
1) [Zebrad](https://github.com/ZcashFoundation/zebra.git)
2) [Lightwalletd](https://github.com/zcash/lightwalletd.git)
3) [Zcashd, Zcash-Cli](https://github.com/zcash/zcash)

### Unit Tests
- To run Unit tests:
1) Simlink or copy compiled `zebrad`, zcashd` and `zcash-cli` binaries to `$ zaino/test_binaries/bins/*`
2) Generate the zcashd chain cache `cargo nextest run generate_zcashd_chain_cache --run-ignored ignored-only`
3) Generate the zebrad chain cache `cargo nextest run generate_zebrad_large_chain_cache --run-ignored ignored-only`
4) Run `$ cargo nextest run tests`

*NOTE: As we currently have several bugs using Zebra's regtest mode for our tests, we are having to rely on loading cached chain-data instead of creating chain data dynamically. Due to this, and the fact that Zebra requires a lock on its chain cache, all unit tests in zaino-state (and any others relying on loading cached chain data) must be run sequentially. This can be done by running tests with the `--no-capture` flag. Eg. `cargo nextest run -p zaino-state --no-capture`.

### Wallet-to-Validator Tests
- To run Wallet-to-Validator tests:
1) Simlink or copy compiled `zcashd`, `zcash-cli` and `zebrad` binaries to `$ zaino/test_binaries/bins/*`
2) Run `$ cargo nextest run --test wallet_to_validator`

### Client RPC Tests
- To run client rpc tests:
1) Simlink or copy compiled `zebrad`, zcashd`, `zcash-cli` and `lightwalletd` binaries to `$ zaino/test_binaries/bins/*`
2) Build release binary `cargo build --release` WARNING: these tests do not use the binary built by cargo nextest
3) Generate the chain cache `cargo nextest run generate_zcashd_chain_cache --run-ignored ignored-only`
4) Run `cargo nextest run --test client_rpcs`

- To run client rpc test `get_subtree_roots_sapling`:
1) sync Zebrad testnet to at least 2 sapling shards
2) copy the Zebrad testnet `state` cache to `zaino/integration-tests/chain_cache/get_subtree_roots_sapling` directory.
See the `get_subtree_roots_sapling` test fixture doc comments in zcash_local_net for more details.

- To run client rpc test `get_subtree_roots_orchard`:
1) sync Zebrad mainnet to at least 2 orchard shards
2) copy the Zebrad mainnet `state` cache to `zaino/integration-tests/chain_cache/get_subtree_roots_orchard` directory.
See the `get_subtree_roots_orchard` test fixture doc comments in zcash_local_net for more details.

