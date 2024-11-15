# Zaino
Zaino is a Rust-implemented indexer for Zcash.

Zaino provides all necessary functionality for "standalone" (formerly "light") clients / wallets, "integrated" (formerly "full") clients / wallets, and block explorers to access both the finalized chain and the non-finalized best chain and mempool, held by either a Zebra or Zcashd full validator.


### Motivations
With the ongoing Zcashd deprecation project, there is a push to transition to a modern, Rust-based software stack for the Zcash ecosystem. By implementing Zaino in Rust, we aim to modernize the codebase, enhance performance, and improve overall security. This work will build on the foundations laid down by [Librustzcash](https://github.com/zcash/librustzcash) and [Zebra](https://github.com/ZcashFoundation/zebra), helping to ensure that the Zcash infrastructure remains robust and maintainable for the future.

Due to current potential data leaks / security weaknesses highlighted in [revised-nym-for-zcash-network-level-privacy](https://forum.zcashcommunity.com/t/revised-nym-for-zcash-network-level-privacy/46688) and [wallet-threat-model](https://zcash.readthedocs.io/en/master/rtd_pages/wallet_threat_model.html), there is a need to use anonymous transport protocols (such as Nym or Tor) to obfuscate clients' identities from Zcash's indexing servers ([Lightwalletd](https://github.com/zcash/lightwalletd), [Zcashd](https://github.com/zcash/zcash), Zaino). As Nym has chosen Rust as their primary SDK ([Nym-SDK](https://github.com/nymtech/nym)), and Tor is currently implementing Rust support ([Arti](https://gitlab.torproject.org/tpo/core/arti)), Rust is an obvious and well-suited choice for this software.

Zebra has been designed to allow direct read access to the finalized state and RPC access to the non-finalized state through its ReadStateService. Integrating directly with this service enables efficient access to chain data and allows new indices to be offered with minimal development.

Separation of validation and indexing functionality serves several purposes. Primarily, removing indexing functionality from the Validator (Zebra) will lead to a smaller and more maintainable codebase. Also, Separating this functionality serves to create a clear trust boundary between the Indexer and Validator, allowing the Indexer to take on this responsibility, this has been the case for "standalone" (formerly "light") clients/wallets using Lightwalletd for some time but is not the case with integrated (formerly "full") client/wallets and block explorers that are currently directly served by Zcashd. Moving all indexing functionality into Zaino will unify this paradigm and simplify Zcash's security model.


### Goals
Our primary goal with Zaino is to serve all non-miner clients, such as wallets and block explorers, in a manner that prioritizes security and privacy while also ensuring the time efficiency critical to a stable currency. We are committed to ensuring that these clients can access all necessary blockchain data and services without exposing sensitive information or being vulnerable to attacks. By implementing robust security measures and privacy protections, Zaino will enable users to interact with the Zcash network confidently and securely.

To facilitate a smooth transition for existing users and developers, Zaino is designed, where possible, to maintain full backward compatibility with Lightwalletd and Zcashd. This means that applications and services currently relying on these platforms can switch to Zaino with minimal adjustments. By providing compatible APIs and interfaces, we aim to reduce friction in adoption and ensure that the broader Zcash ecosystem can benefit from Zaino's enhancements without significant rewrites or learning curves.

### Scope
Zaino will implement a comprehensive RPC API to serve all non-miner client requests effectively. This API will encompass all functionality currently in the LightWallet gRPC service ([CompactTxStreamer](https://github.com/zcash/librustzcash/blob/main/zcash_client_backend/proto/service.proto)), currently served by Lightwalletd, and a subset of the [Zcash RPC](https://zcash.github.io/rpc/)s required by wallets and block explorers, currently served by Zcashd. Zaino will unify these two RPC services and provide a single, straightforward interface for Zcash clients and service providers to access the data and services they require.

In addition to the RPC API, Zaino will offer a client library allowing developers to integrate Zaino's functionality directly into their Rust applications. Along with the RemoteReadStateService mentioned below, this will allow both local and remote access to the data and services provided by Zaino without the overhead of using an RPC protocol, and also allows Zebra to stay insulated from directly interfacing with client software.

Zaino will extend the functionality of Zebra's ReadStateService, using a Hyper wrapper, to offer remote read access to the chain state data held by Zebra. This will primarily be designed as a remote link between Zebra and Zaino and it is not intended for developers to directly interface with this service, but to instead use functionality exposed by the client library in Zaino. This will allow a diverse range of setup options when running Zebra, Zaino, and external client code, and will also allow developers to integrate remote chain access directly into their code, as mentioned above.


## Documentation
- [Use Cases](./docs/use_cases.md): Holds instructions and example use cases.
- [Testing](./docs/testing.md): Hold intructions fo running tests.
- [System Architecture](./docs/system_architecture.pdf): Holds the Zcash system architecture diagrams.
- [Internal Architecture](./docs/internal_architecture.pdf): Holds the internal Zaino system architecture diagrams.
- [Internal Specification](./docs/internal_spec.md): Holds a specification for Zaino and its crates, detailing and their functionality, interfaces and dependencies.
- [RPC API Spec](./docs/rpc_api.md): Holds a full specification of all of the RPC services served by Zaino.
- [Cargo Docs](./docs/index.md): Holds a full code specification for Zaino.


## Security Vulnerability Disclosure
If you believe you have discovered a security issue, please contact us at:

zingodisclosure@proton.me


## License
This project is licensed under the [Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0). See the [LICENSE](./LICENSE) file for details.
