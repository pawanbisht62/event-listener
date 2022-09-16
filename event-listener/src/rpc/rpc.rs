// Copyright 2019-2022 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

//! RPC types and client for interacting with a substrate node.
//!
//! This is used behind the scenes by various `subxt` APIs, but can
//! also be used directly.
//!
use super::{
    rpc_params,
    RpcClient,
    RpcClientT,
    Subscription,
};
use crate::{
    error::Error,
    utils::PhantomDataSendSync,
    Config,
    Metadata,
};
use codec::{
    Decode,
};
use frame_metadata::RuntimeMetadataPrefixed;
use serde::{
    Deserialize,
    Serialize,
};
use sp_core::{
    storage::StorageData,
    Bytes,
    U256,
};
use std::collections::HashMap;

/// A number type that can be serialized both as a number or a string that encodes a number in a
/// string.
///
/// We allow two representations of the block number as input. Either we deserialize to the type
/// that is specified in the block type or we attempt to parse given hex value.
///
/// The primary motivation for having this type is to avoid overflows when using big integers in
/// JavaScript (which we consider as an important RPC API consumer).
#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum NumberOrHex {
    /// The number represented directly.
    Number(u64),
    /// Hex representation of the number.
    Hex(U256),
}

/// Wrapper for NumberOrHex to allow custom From impls
#[derive(Serialize)]
pub struct BlockNumber(NumberOrHex);

/// Possible transaction status events.
///
/// # Note
///
/// This is copied from `sp-transaction-pool` to avoid a dependency on that crate. Therefore it
/// must be kept compatible with that type from the target substrate version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubstrateTxStatus<Hash, BlockHash> {
    /// Transaction is part of the future queue.
    Future,
    /// Transaction is part of the ready queue.
    Ready,
    /// The transaction has been broadcast to the given peers.
    Broadcast(Vec<String>),
    /// Transaction has been included in block with given hash.
    InBlock(BlockHash),
    /// The block this transaction was included in has been retracted.
    Retracted(BlockHash),
    /// Maximum number of finality watchers has been reached,
    /// old watchers are being removed.
    FinalityTimeout(BlockHash),
    /// Transaction has been finalized by a finality-gadget, e.g GRANDPA
    Finalized(BlockHash),
    /// Transaction has been replaced in the pool, by another transaction
    /// that provides the same tags. (e.g. same (sender, nonce)).
    Usurped(Hash),
    /// Transaction has been dropped from the pool because of the limit.
    Dropped,
    /// Transaction is no longer valid in the current state.
    Invalid,
}

/// This contains the runtime version information necessary to make transactions, as obtained from
/// the RPC call `state_getRuntimeVersion`,
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeVersion {
    /// Version of the runtime specification. A full-node will not attempt to use its native
    /// runtime in substitute for the on-chain Wasm runtime unless all of `spec_name`,
    /// `spec_version` and `authoring_version` are the same between Wasm and native.
    pub spec_version: u32,

    /// All existing dispatches are fully compatible when this number doesn't change. If this
    /// number changes, then `spec_version` must change, also.
    ///
    /// This number must change when an existing dispatchable (module ID, dispatch ID) is changed,
    /// either through an alteration in its user-level semantics, a parameter
    /// added/removed/changed, a dispatchable being removed, a module being removed, or a
    /// dispatchable/module changing its index.
    ///
    /// It need *not* change when a new module is added or when a dispatchable is added.
    pub transaction_version: u32,

    /// The other fields present may vary and aren't necessary for `subxt`; they are preserved in
    /// this map.
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

/// Client for substrate rpc interfaces
pub struct Rpc<T: Config> {
    client: RpcClient,
    _marker: PhantomDataSendSync<T>,
}

impl<T: Config> Clone for Rpc<T> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            _marker: PhantomDataSendSync::new(),
        }
    }
}

// Expose subscribe/request, and also subscribe_raw/request_raw
// from the even-deeper `dyn RpcClientT` impl.
impl<T: Config> std::ops::Deref for Rpc<T> {
    type Target = RpcClient;
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl<T: Config> Rpc<T> {
    /// Create a new [`Rpc`]
    pub fn new<R: RpcClientT>(client: R) -> Self {
        Self {
            client: RpcClient::new(client),
            _marker: PhantomDataSendSync::new(),
        }
    }

    /// Fetch the raw bytes for a given storage key
    pub async fn storage(
        &self,
        key: &[u8],
        hash: Option<T::Hash>,
    ) -> Result<Option<StorageData>, Error> {
        let params = rpc_params![to_hex(key), hash];
        let data = self.client.request("state_getStorage", params).await?;
        Ok(data)
    }

    /// Fetch the metadata
    pub async fn metadata(&self) -> Result<Metadata, Error> {
        let bytes: Bytes = self
            .client
            .request("state_getMetadata", rpc_params![])
            .await?;
        let meta: RuntimeMetadataPrefixed = Decode::decode(&mut &bytes[..])?;
        let metadata: Metadata = meta.try_into()?;
        Ok(metadata)
    }

    /// Get a block hash, returns hash of latest block by default
    pub async fn block_hash(
        &self,
        block_number: Option<BlockNumber>,
    ) -> Result<Option<T::Hash>, Error> {
        let params = rpc_params![block_number];
        let block_hash = self.client.request("chain_getBlockHash", params).await?;
        Ok(block_hash)
    }

    /// Fetch the runtime version
    pub async fn runtime_version(
        &self,
        at: Option<T::Hash>,
    ) -> Result<RuntimeVersion, Error> {
        let params = rpc_params![at];
        let version = self
            .client
            .request("state_getRuntimeVersion", params)
            .await?;
        Ok(version)
    }

    /// Subscribe to blocks.
    pub async fn subscribe_blocks(&self) -> Result<Subscription<T::Header>, Error> {
        let subscription = self
            .client
            .subscribe(
                "chain_subscribeNewHeads",
                rpc_params![],
                "chain_unsubscribeNewHeads",
            )
            .await?;

        Ok(subscription)
    }
}

fn to_hex(bytes: impl AsRef<[u8]>) -> String {
    format!("0x{}", hex::encode(bytes.as_ref()))
}

#[cfg(test)]
mod test {
    use super::*;

    /// A util function to assert the result of serialization and deserialization is the same.
    pub(crate) fn assert_deser<T>(s: &str, expected: T)
    where
        T: std::fmt::Debug
            + serde::ser::Serialize
            + serde::de::DeserializeOwned
            + PartialEq,
    {
        assert_eq!(serde_json::from_str::<T>(s).unwrap(), expected);
        assert_eq!(serde_json::to_string(&expected).unwrap(), s);
    }

    #[test]
    fn test_deser_runtime_version() {
        let val: RuntimeVersion = serde_json::from_str(
            r#"{
            "specVersion": 123,
            "transactionVersion": 456,
            "foo": true,
            "wibble": [1,2,3]
        }"#,
        )
        .expect("deserializing failed");

        let mut m = std::collections::HashMap::new();
        m.insert("foo".to_owned(), serde_json::json!(true));
        m.insert("wibble".to_owned(), serde_json::json!([1, 2, 3]));

        assert_eq!(
            val,
            RuntimeVersion {
                spec_version: 123,
                transaction_version: 456,
                other: m
            }
        );
    }

    #[test]
    fn should_serialize_and_deserialize() {
        assert_deser(r#""0x1234""#, NumberOrHex::Hex(0x1234.into()));
        assert_deser(r#""0x0""#, NumberOrHex::Hex(0.into()));
        assert_deser(r#"5"#, NumberOrHex::Number(5));
        assert_deser(r#"10000"#, NumberOrHex::Number(10000));
        assert_deser(r#"0"#, NumberOrHex::Number(0));
        assert_deser(r#"1000000000000"#, NumberOrHex::Number(1000000000000));
    }
}
