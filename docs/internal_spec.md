# Zaino
The Zaino repo consists of several crates that collectively provide an indexing service and APIs for the Zcash blockchain. The crates are modularized to separate concerns, enhance maintainability, and allow for flexible integration.

The main crates are:
  - `Zainod`
  - `Zaino-Serve`
  - `Zaino-State`
  - `Zaino-Fetch`
  - `Zaino-Proto`

### Workspace Dependencies
  - `zcash_protocol`
  - `zebra-chain`
  - `zebra-rpc`
  - `tokio`
  - `tonic`
  - `http`
  - `thiserror`

Below is a detailed specification for each crate.

A full specification of the public functionality and RPC services available in Zaino is availabe in [Cargo Docs](https://zingolabs.github.io/zaino/index.html) and [RPC API Spec](./rpc_api.md).


## ZainoD
`ZainoD` is the main executable that runs the Zaino indexer gRPC service. It serves as the entry point for deploying the Zaino service, handling configuration and initialization of the server components.

### Functionality
- Service Initialization:
  - Parses command-line arguments and configuration files.
  - Initializes the gRPC server and internal caching systems using components from `zaino-serve` and `zaino-state` (backed by `zaino-fetch`).
  - Sets up logging and monitoring systems.

- Runtime Management:
  - Manages the asynchronous runtime using `Tokio`.
  - Handles graceful shutdowns and restarts.

### Interfaces
- Executable Interface:
  - Provides a CLI for configuring the service. (Currently it is only possible to set the conf file path.)

- Configuration Files:
  - Supports TOML files for complex configurations.

### Dependencies
  - `zaino-fetch`
  - `zaino-serve`
  - `tokio`
  - `http`
  - `thiserror`
  - `serde`
  - `ctrlc`
  - `toml`
  - `clap`

Full documentation for `ZainoD` can be found [here](https://zingolabs.github.io/zaino/zainod/index.html) and [here](https://zingolabs.github.io/zaino/zainodlib/index.html).


## Zaino-Serve
`Zaino-Serve` contains the gRPC server and the Rust implementations of the LightWallet gRPC service (`CompactTxStreamerServer`). It handles incoming client requests and interacts with backend services to fulfill them.

### Functionality
- gRPC Server Implementation:
  - Utilizes `Tonic` to implement the gRPC server.
  - Uses a `Director-Ingestor-Worker` model (see [Internal Architecture](./internal_architecture.pdf)) to allow the addition of Nym or Tor based `Ingestors`.
  - Dynamically manages the internal Worker pool and Request queue and active Ingestors, handling errors and restarting services where necessary.
  - Hosts the `CompactTxStreamerServer` service for client interactions.

- `CompactTxStreamerServer` Method Implementations:
  - Implements the full set of methods as defined in the [LightWallet Protocol](https://github.com/zcash/librustzcash/blob/main/zcash_client_backend/proto/service.proto).

- Request Handling:
  - Validates and parses client requests.
  - Communicates with `zaino-state` or `zaino-fetch` to retrieve data.

- Error Handling:
  - Maps internal errors to appropriate gRPC status codes.
  - Provides meaningful error messages to clients.

### Interfaces
- Public gRPC API:
  - Defined in `zaino-proto` and exposed to clients.

- Internal Library:
  - The `server::director` module provides the following gRPC server management functions: `ServerStatus::new`, `ServerStatus::load`, `Server::spawn`, `Server::serve`, `Server::check_for_shutdown`, `Server::shutdown`, `Server::status`, `Server::statustype`, `Server::statuses`, `Server::check_statuses`.

### Dependencies
  - `zaino-proto`
  - `zaino-fetch`
  - `zebra-chain`
  - `zebra-rpc`
  - `tokio`
  - `tonic`
  - `http`
  - `thiserror`
  - `prost`
  - `hex`
  - `tokio-stream`
  - `futures`
  - `async-stream`
  - `crossbeam-channel`
  - `lazy-regex`
  - `whoami`

Full documentation for `Zaino-Serve` can be found [here](https://zingolabs.github.io/zaino/zaino_serve/index.html).


## Zaino-State
`Zaino-State` is a library that provides access to the mempool and blockchain data by interfacing directly with `Zebra`'s `ReadStateService`. It is designed for direct consumption by full node wallets and internal services. (Currently unimplemented.)

### Functionality
- Blockchain Data Access:
  - Fetches finalized and non-finalized state data.
  - Retrieves transaction data and block headers.
  - Accesses chain metadata like network height and difficulty.

- Mempool Management:
  - Interfaces with the mempool to fetch pending transactions.
  - Provides efficient methods to monitor mempool changes.

- Chain Synchronization:
  - Keeps track of the chain state in sync with Zebra.
  - Handles reorgs and updates to the best chain.

Caching Mechanisms:
  - Implements caching for frequently accessed data to improve performance.

### Interfaces
- Public Library API:
  - Provides data retrieval and submission functions that directly correspond to the RPC services offered by `zaino-serve`.
  - Provides asynchronous interfaces compatible with `Tokio`.

- Event Streams:
  - Offers highly concurrent, lock-free streams or channels to subscribe to blockchain events.

### Dependencies
  - `zebra-state`
  - `tokio`
  - `thiserror`

Full documentation for `Zaino-State` can be found [here](https://zingolabs.github.io/zaino/zaino_state/index.html).


## Zaino-Fetch
`Zaino-Fetch` is a library that provides access to the mempool and blockchain data using Zebra's RPC interface. It is primarily used as a backup and for backward compatibility with systems that rely on RPC communication such as `Zcashd`.

### Functionality
- RPC Client Implementation:
  - Implements a `JSON-RPC` client to interact with `Zebra`'s RPC endpoints.
  - Handles serialization and deserialization of RPC calls.

- Data Retrieval and Transaction Submission:
  - Fetches blocks, transactions, and mempool data via RPC.
  - Sends transactions to the network using the `sendrawtransaction` RPC method.

- Mempool and CompactFormat access:
  - Provides a simple mempool implementation for use in gRPC service implementations. (This is due to be refactored and possibly moved with the development of `Zaino-State`.)
  - Provides parse implementations for converting "full" blocks and transactions to "compact" blocks and transactions.

- Fallback Mechanism:
  - Acts as a backup when direct access via `zaino-state` is unavailable.

### Interfaces
- Internal API:
  - The `jsonrpc::connector` module provides the following JSON-RPC client management functions: `new`, `uri`, `url`, `test_node_and_return_uri`.
  - The `jsonrpc::connector` module provides the following data retrieval and submission functions: `get_info`, `get_blockchain_info`, `get_address_balance`, `send_raw_transaction`, `get_block`, `get_raw_mempool`, `get_treestate`, `get_subtrees_by_index`, `get_raw_transaction`, `get_address_txids`, `get_address_utxos`. (This may be expanded to match the set of Zcash RPC's that Zaino is taking over from Zcashd.)
  - The `chain::block` module provides the following block parsing and fetching functions: `get_block_from_node`, `get_nullifiers_from_node`, `FullBlock::parse_from_hex`, `FullBlock::to_compact`, FullBlock::header, FullBlock::transactions, FullBlock::Height, FullBlockHeader::version, FullBlockHeader::hash_prev_block, FullBlockHeader::hash_merkle_root, FullBlockHeader::time, FullBlockHeader::n_bits_bytes, FullBlockHeader::nonce, FullBlockHeader::solution, FullBlockHeader::cached_hash.
  The `chain::transaction` module provides the following transaction parsing and fetching functions: `FullTransaction::f_overwintered`, `FullTransaction::version`, `FullTransaction::n_version_group_id`, `FullTransaction::consensus_branch_id`, `FullTransaction::transparent_inputs`, `FullTransaction::transparent_outputs`, `FullTransaction::shielded_spends`, `FullTransaction::shielded_outputs`, `FullTransaction::join_splits`, `FullTransaction::orchard_actions`, `FullTransaction::raw_bytes`, `FullTransaction::tx_id`, `FullTransaction::to_compact`.
  - The `chain::mempool` module provides the following mempool management and fetching functions: `new`, `update`, `get_mempool_txids`, `get_filtered_mempool_txids`, `get_best_block_hash`. (This is due to be refactored and possibly moved with the development of `Zaino-State`.)
  - Designed to be used by `zaino-serve` transparently.

### Dependencies
  - `zaino-proto`
  - `zcash_protocol`
  - `zebra-chain`
  - `zebra-rpc`
  - `tokio`
  - `tonic`
  - `http`
  - `thiserror`
  - `prost`
  - `reqwest`
  - `url`
  - `serde_json`
  - `serde`
  - `hex`
  - `indexmap`
  - `base64`
  - `byteorder`
  - `sha2`

Full documentation for `Zaino-Fetch` can be found [here](https://zingolabs.github.io/zaino/zaino_fetch/index.html).


## Zaino-Proto
`Zaino-Proto` contains the `Tonic`-generated code for the LightWallet service RPCs and compact formats. It holds the protocol buffer definitions and the generated Rust code necessary for gRPC communication.

### Functionality
- Protocol Definitions:
  - `.proto` files defining the services and messages for LightWalletd APIs.
  - Includes definitions for compact blocks, transactions, and other data structures.

- Code Generation:
  - Uses `prost` to generate Rust types from `.proto` files.
  - Generates client and server stubs for gRPC services.

### Interfaces
- Generated Code:
  - Provides Rust modules that can be imported by other crates.
  - Exposes types and traits required for implementing gRPC services.

### Dependencies
  - `tonic`
  - `prost`
  - `tonic-build`
  - `which`

* We plan to eventually rely on `LibRustZcash`'s versions but hold our own here for development purposes.


## Zaino-Testutils and Integration-Tests
The `Zaino-Testutils` and `Integration-Tests` crates are dedicated to testing the Zaino project. They provide utilities and comprehensive tests to ensure the correctness, performance, and reliability of Zaino's components.
- `Zaino-Testutils`: This crate contains common testing utilities and helper functions used across multiple test suites within the Zaino project.
- `Integration-Tests`: This crate houses integration tests that validate the interaction between different Zaino components and external services like `Zebra` and `Zingolib`.

### Test Modules
- `integrations`: Holds Wallet-to-Validator tests that test Zaino's functionality within the compete software stack.
- `client_rpcs`: Holds RPC tests that test the functionality of the LightWallet gRPC services in Zaino and compares the outputs with the corresponding services in `Lightwalletd` to ensure compatibility.

### Dependencies
  - `zaino-fetch`
  - `zainod`
  - `zingolib`
  - `zaino-testutils`
  - `zcash_local_net`
  - `tokio`
  - `tonic`
  - `http`
  - `ctrlc`
  - `tempfile`
  - `portpicker`
  - `tracing-subscriber`
  - `once_cell`

Full documentation for `Zaino-Testutils` can be found [here](https://zingolabs.github.io/zaino/zaino_testutils/index.html).
