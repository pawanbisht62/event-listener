// Copyright 2019-2022 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

//! Types representing the errors that can be returned.
use core::fmt::Debug;

// Re-expose the errors we use from other crates here:
pub use crate::metadata::{
    InvalidMetadataError,
    MetadataError,
};
pub use scale_value::scale::{
    DecodeError,
    EncodeError,
};
pub use sp_core::crypto::SecretStringError;
pub use sp_runtime::transaction_validity::TransactionValidityError;

/// The underlying error enum, generic over the type held by the `Runtime`
/// variant. Prefer to use the [`Error<E>`] and [`Error`] aliases over
/// using this type directly.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Codec error.
    #[error("Scale codec error: {0}")]
    Codec(#[from] codec::Error),
    /// Rpc error.
    #[error("Rpc error: {0}")]
    Rpc(#[from] RpcError),
    /// Serde serialization error
    #[error("Serde json error: {0}")]
    Serialization(#[from] serde_json::error::Error),
    /// Extrinsic validity error
    #[error("Transaction Validity Error: {0:?}")]
    Invalid(TransactionValidityError),
    /// Invalid metadata error
    #[error("Invalid Metadata: {0}")]
    InvalidMetadata(#[from] InvalidMetadataError),
    /// Invalid metadata error
    #[error("Metadata: {0}")]
    Metadata(#[from] MetadataError),
    /// Error decoding to a [`crate::dynamic::Value`].
    #[error("Error decoding into dynamic value: {0}")]
    DecodeValue(#[from] DecodeError),
    /// Error encoding from a [`crate::dynamic::Value`].
    #[error("Error encoding from dynamic value: {0}")]
    EncodeValue(#[from] EncodeError<()>),
    /// Other error.
    #[error("Other error: {0}")]
    Other(String),
}

impl From<String> for Error {
    fn from(error: String) -> Self {
        Error::Other(error)
    }
}

/// An RPC error. Since we are generic over the RPC client that is used,
/// the error is any custom string.
#[derive(Debug, thiserror::Error)]
#[error("RPC error: {0}")]
pub struct RpcError(pub String);


