# Indexer Live Service
### Dependencies
1) [Zebrad](https://github.com/ZcashFoundation/zebra.git) or [Zcashd, Zcash-Cli](https://github.com/zcash/zcash.git)
2) [Zingolib](https://github.com/zingolabs/zingolib.git) [if running Zingo-Cli]

### Running ZainoD
- To run a Zaino server, backed locally by Zebrad first build Zaino:
1) Run `$ cargo build --release`
2) Add compiled binary held at `#PATH_TO/zaino/target/release/zainod` to PATH.

- Then to launch Zaino: [in seperate terminals]:
1) Run `$ zebrad --config #PATH_TO_CONF/zebrad.toml start`
2) Run `$ zainod --config #PATH_TO_CONF/zindexer.toml` 

- To launch Zingo-Cli running through Zaino [from #PATH_TO/zingolib]:
1) Run `$ cargo run --release --package zingo-cli -- --chain "CHAIN_TYPE" --server "ZAINO_LISTEN_ADDR" --data-dir #PATH_TO_WALLET_DATA_DIR`

- Example Config files for running Zebra and Zaino on testnet are given in `zaino/zainod/*`

Note:
Configuration data can be set using a .toml file (an example zindexer.toml is given in zingo-indexer/zindexer.toml) and can be set at runtime using the --config arg:
- Run `$ cargo run --config zingo-indexerd/zindexer.toml`



# Local Library
**Currently Unimplemented, documentation will be added here as this functionality becomes available.**


# Remote Library
**Currently Unimplemented, documentation will be added here as this functionality becomes available.**


