searchState.loadedDescShard("zaino_serve", 0, "Holds a gRPC server capable of servicing clients over TCP.\nLightwallet service RPC implementations.\nZaino’s gRPC server implementation.\nConfiguration data for gRPC server.\nReturns the argument unchanged.\nReturns all unspent outputs for a list of addresses.\nReturns all unspent outputs for a list of addresses.\nReturn the compact block corresponding to the given block …\nSame as GetBlock except actions contain only nullifiers.\nReturn a list of consecutive compact blocks.\nSame as GetBlockRange except actions contain only …\nReturn the height of the tip of the best chain.\nGetLatestTreeState returns the note commitment tree state …\nReturn information about this lightwalletd instance and …\nReturn a stream of current Mempool transactions. This will …\nReturn the compact transactions currently in the mempool; …\nReturns a stream of information about roots of subtrees of …\nReturns the total balance for a list of taddrs\nReturns the total balance for a list of taddrs\nThis name is misleading, returns the full transactions …\nReturn the requested full (not compact) transaction (as …\nGetTreeState returns the note commitment tree state …\nCalls <code>U::from(self)</code>.\nLightwalletd uri. Used by grpc_passthrough to pass on …\nRepresents the Online status of the gRPC server.\nTesting-only, requires lightwalletd –ping-very-insecure …\nSubmit the given transaction to the Zcash network.\nLightwallet service RPC implementations.\nZebrad uri.\nStream of CompactBlocks, output type of get_block_range.\nStream of RawTransactions, output type of …\nStream of RawTransactions, output type of …\nStream of CompactBlocks, output type of get_block_range.\nStream of CompactBlocks, output type of get_block_range.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nReturns new instanse of RawTransactionStream.\nReturns new instanse of RawTransactionStream.\nReturns new instanse of CompactBlockStream.\nReturns new instanse of CompactBlockStream.\nReturns new instanse of CompactBlockStream.\nHolds a thread safe reperesentation of a StatusType. …\nRunning shutdown routine.\nOffline.\nOn hold, due to blockcache / node error.\nWaiting for requests from the queue.\nOffline.\nRunning initial startup routine.\nStatus of the server’s components.\nProcessing requests from the queue.StatusType\nZingo-Indexer gRPC server.\nHold error types for the server and related functionality.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nLoads the value held in the AtomicStatus\nCreates a new AtomicStatus\nRequest types.\nSets the value held in the AtomicStatus\nLightWallet server capable of servicing clients over TCP.\nHolds the status of the server and all its components.\nChecks indexers online status and servers internal status …\nChecks statuses, handling errors.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nReturns the ServerStatus.\nCreates a ServerStatus.\nRepresents the Online status of the Server.\nStarts the gRPC service.\nStatus of the Server.\nSets the servers to close gracefully.\nSpawns a new Server.\nReturns the servers current status usize.\nUpdates and returns the status of the server and its parts.\nReturns the servers current statustype.\nTcp listener based error.\nErrors originating from incorrect enum types being called.\nZingo-Indexer ingestor errors.\nIngestor based errors.\nReturned when a worker or dispatcher tries to receive from …\nReturned when a worker or dispatcher tries to receive from …\nZingo-Indexer queue errors.\nReturned when a requests is pushed to a full queue.\nError from failing to send new request to the queue.\nZingo-Indexer request errors.\nRequest based errors.\nRequest based errors.\nServer configuration errors.\nZingo-Indexer server errors.\nSystem time errors.\nTokio join error.\nTonic transport error.\nZingo-Indexer worker errors.\nWorker based errors.\nWorker Pool Full.\nWorker Pool at idle.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nTcpStream holing an incoming gRPC request.\nRequests originating from the Tcp server.\nRequests originating from the gRPC server.\nZingo-Indexer request, used by request queue.\nReturns the duration sunce the request was received.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the underlying request.\nReturns the underlying TcpStream help by the request\nIncreases the requeue attempts for the request.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCreates a ZingoIndexerRequest from a gRPC service call, …\nReturns the number of times the request has been requeued.")