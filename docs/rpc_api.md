# Zaino RPC APIs
Zaino's Final RPC API is currently unfinalised and will be completed in this [GitHub Issue](https://github.com/zingolabs/zaino/issues/69).


## Lightwallet gRPC Services
Zaino Currently Serves the following gRPC services as defined in the [LightWallet Protocol](https://github.com/zcash/librustzcash/blob/main/zcash_client_backend/proto/service.proto):
  - GetLatestBlock (ChainSpec) returns (BlockID)
  - GetBlock (BlockID) returns (CompactBlock)
  - GetBlockNullifiers (BlockID) returns (CompactBlock)
  - GetBlockRange (BlockRange) returns (stream CompactBlock)
  - GetBlockRangeNullifiers (BlockRange) returns (stream CompactBlock)
  - GetTransaction (TxFilter) returns (RawTransaction)
  - SendTransaction (RawTransaction) returns (SendResponse)
  - GetTaddressTxids (TransparentAddressBlockFilter) returns (stream RawTransaction)
  - GetTaddressBalance (AddressList) returns (Balance)
  - GetTaddressBalanceStream (stream Address) returns (Balance) (**MARKED FOR DEPRECATION**)
  - GetMempoolTx (Exclude) returns (stream CompactTx)
  - GetMempoolStream (Empty) returns (stream RawTransaction)
  - GetTreeState (BlockID) returns (TreeState)
  - GetLatestTreeState (Empty) returns (TreeState)
  - GetSubtreeRoots (GetSubtreeRootsArg) returns (stream SubtreeRoot)
  - GetAddressUtxos (GetAddressUtxosArg) returns (GetAddressUtxosReplyList)
  - GetAddressUtxosStream (GetAddressUtxosArg) returns (stream GetAddressUtxosReply)
  - GetLightdInfo (Empty) returns (LightdInfo)
  - Ping (Duration) returns (PingResponse) (**CURRENTLY UNIMPLEMENTED**)


## Zcash RPC Services
Zaino has committed to taking over responsibility for serving the following [Zcash RPCs](https://zcash.github.io/rpc/) from Zcashd:
  - [getaddressbalance](https://zcash.github.io/rpc/getaddressbalance.html)
  - [getaddressdeltas](https://zcash.github.io/rpc/getaddressdeltas.html)
  - [getaddressmempool](https://zcash.github.io/rpc/getaddressmempool.html) (**MARKED FOR DEPRECATION**)
  - [getaddresstxids](https://zcash.github.io/rpc/getaddresstxids.html)
  - [getaddressutxos](https://zcash.github.io/rpc/getaddressutxos.html)
  - [getbestblockhash](https://zcash.github.io/rpc/getbestblockhash.html) (**LOW PRIORITY. MARKED FOR POSSIBLE DEPRECATION**)
  - [getblock](https://zcash.github.io/rpc/getblock.html)
  - [getblockchaininfo](https://zcash.github.io/rpc/getblockchaininfo.html)
  - [getblockcount](https://zcash.github.io/rpc/getblockcount.html)
  - [getblockdeltas](https://zcash.github.io/rpc/getblockdeltas.html)
  - [getblockhash](https://zcash.github.io/rpc/getblockhash.html)
  - [getblockhashes](https://zcash.github.io/rpc/getblockhashes.html)
  - [getblockheader](https://zcash.github.io/rpc/getblockheader.html)
  - [getchaintips](https://zcash.github.io/rpc/getchaintips.html)
  - [getdifficulty](https://zcash.github.io/rpc/getdifficulty.html)
  - [getmempoolinfo](https://zcash.github.io/rpc/getmempoolinfo.html)
  - [getrawmempool](https://zcash.github.io/rpc/getrawmempool.html)
  - [getspentinfo](https://zcash.github.io/rpc/getspentinfo.html)
  - [gettxout](https://zcash.github.io/rpc/gettxout.html)
  - [gettxoutproof](https://zcash.github.io/rpc/gettxoutproof.html) (**LOW PRIORITY. MARKED FOR POSSIBLE DEPRECATION**)
  - [gettxoutsetinfo](https://zcash.github.io/rpc/gettxoutsetinfo.html)
  - [verifytxoutproof](https://zcash.github.io/rpc/verifytxoutproof.html)(**LOW PRIORITY. MARKED FOR POSSIBLE DEPRECATION**)
  - [z_gettreestate](https://zcash.github.io/rpc/z_gettreestate.html)

Zaino will also provide wrapper functionality for the following RPCs in Zebra (to allow block explorers to fetch all data they require directly from / through Zaino):
  - [getinfo](https://zcash.github.io/rpc/getinfo.html)
  - [getmininginfo](https://zcash.github.io/rpc/getmininginfo.html)
  - [getnetworksolps](https://zcash.github.io/rpc/getnetworksolps.html)
  - [getnetworkinfo](https://zcash.github.io/rpc/getnetworkinfo.html)
  - [getpeerinfo](https://zcash.github.io/rpc/getpeerinfo.html)
  - [ping](https://zcash.github.io/rpc/ping.html)


