// Copyright 2019-2022 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

//! This module provides a [`Config`] type, which is used to define various
//! types that are important in order to speak to a particular chain.
//! [`SubstrateConfig`] provides a default set of these types suitable for the
//! default Substrate node implementation, and [`PolkadotConfig`] for a
//! Polkadot node.

use codec::{
    Codec,
    Encode,
    EncodeLike,
};
use core::fmt::Debug;
use sp_runtime::traits::{
    AtLeast32Bit,
    Hash,
    Header,
    MaybeSerializeDeserialize,
    Member,
    Verify,
};

/// Runtime types.
// Note: the 'static bound isn't strictly required, but currently deriving TypeInfo
// automatically applies a 'static bound to all generic types (including this one),
// and so until that is resolved, we'll keep the (easy to satisfy) constraint here.
pub trait Config: 'static {
    /// Account index (aka nonce) type. This stores the number of previous
    /// transactions associated with a sender account.
    type Index: Parameter
        + Member
        + serde::de::DeserializeOwned
        + Default
        + AtLeast32Bit
        + Copy
        + scale_info::TypeInfo
        + Into<u64>;

    /// The block number type used by the runtime.
    type BlockNumber: Parameter
        + Member
        + Default
        + Copy
        + core::hash::Hash
        + core::str::FromStr
        + Into<u64>;

    /// The output of the `Hashing` function.
    type Hash: Parameter
        + Member
        + MaybeSerializeDeserialize
        + Ord
        + Default
        + Copy
        + std::hash::Hash
        + AsRef<[u8]>
        + AsMut<[u8]>
        + scale_info::TypeInfo;

    /// The hashing system (algorithm) being used in the runtime (e.g. Blake2).
    type Hashing: Hash<Output = Self::Hash>;

    /// The user account identifier type for the runtime.
    type AccountId: Parameter + Member + serde::Serialize;

    /// The address type. This instead of `<frame_system::Trait::Lookup as StaticLookup>::Source`.
    type Address: Codec + Clone + PartialEq;

    /// The block header.
    type Header: Parameter
        + Header<Number = Self::BlockNumber, Hash = Self::Hash>
        + serde::de::DeserializeOwned;

    /// Signature type.
    type Signature: Verify + Encode + Send + Sync + 'static;

}

/// Parameter trait copied from `substrate::frame_support`
pub trait Parameter: Codec + EncodeLike + Clone + Eq + Debug {}
impl<T> Parameter for T where T: Codec + EncodeLike + Clone + Eq + Debug {}

/// Default set of commonly used types by Substrate runtimes.
// Note: We only use this at the type level, so it should be impossible to
// create an instance of it.
pub enum SubstrateConfig {}

impl Config for SubstrateConfig {
    type Index = u32;
    type BlockNumber = u32;
    type Hash = sp_core::H256;
    type Hashing = sp_runtime::traits::BlakeTwo256;
    type AccountId = sp_runtime::AccountId32;
    type Address = sp_runtime::MultiAddress<Self::AccountId, u32>;
    type Header =
        sp_runtime::generic::Header<Self::BlockNumber, sp_runtime::traits::BlakeTwo256>;
    type Signature = sp_runtime::MultiSignature;
}

/// Default set of commonly used types by Polkadot nodes.
pub type PolkadotConfig = WithExtrinsicParams<
    SubstrateConfig,
>;

/// Take a type implementing [`Config`] (eg [`SubstrateConfig`])
///
/// # Example
///
/// ```
/// use subxt::config::{ SubstrateConfig, WithExtrinsicParams };
/// use subxt::tx::PolkadotExtrinsicParams;
///
/// // This is how PolkadotConfig is implemented:
/// type PolkadotConfig = WithExtrinsicParams<SubstrateConfig, PolkadotExtrinsicParams<SubstrateConfig>>;
/// ```
pub struct WithExtrinsicParams<
    T: Config,
> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: Config> Config for WithExtrinsicParams<T>
{
    type Index = T::Index;
    type BlockNumber = T::BlockNumber;
    type Hash = T::Hash;
    type Hashing = T::Hashing;
    type AccountId = T::AccountId;
    type Address = T::Address;
    type Header = T::Header;
    type Signature = T::Signature;
}
