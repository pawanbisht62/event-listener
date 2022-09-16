// Copyright 2019-2022 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use crate::{
    events::EventsClient,
    rpc::RuntimeVersion,
    Config,
    Metadata,
};
use derivative::Derivative;
use std::sync::Arc;

/// A trait representing a client that can perform
/// offline-only actions.
pub trait OfflineClientT<T: Config>: Clone + Send + Sync + 'static {
    /// Return the provided [`Metadata`].
    fn metadata(&self) -> Metadata;
    /// Return the provided [`RuntimeVersion`].
    fn runtime_version(&self) -> RuntimeVersion;

    /// Work with events.
    fn events(&self) -> EventsClient<T, Self> {
        EventsClient::new(self.clone())
    }
}

/// A client that is capable of performing offline-only operations.
/// Can be constructed as long as you can populate the required fields.
#[derive(Derivative)]
#[derivative(Debug(bound = ""), Clone(bound = ""))]
pub struct OfflineClient<T: Config> {
    inner: Arc<Inner<T>>,
}

#[derive(Derivative)]
#[derivative(Debug(bound = ""), Clone(bound = ""))]
struct Inner<T: Config> {
    genesis_hash: T::Hash,
    runtime_version: RuntimeVersion,
    metadata: Metadata,
}

impl<T: Config> OfflineClient<T> {

    /// Return the runtime version.
    pub fn runtime_version(&self) -> RuntimeVersion {
        self.inner.runtime_version.clone()
    }

    /// Return the [`Metadata`] used in this client.
    pub fn metadata(&self) -> Metadata {
        self.inner.metadata.clone()
    }

}

impl<T: Config> OfflineClientT<T> for OfflineClient<T> {
    fn runtime_version(&self) -> RuntimeVersion {
        self.runtime_version()
    }
    fn metadata(&self) -> Metadata {
        self.metadata()
    }
}

// For ergonomics; cloning a client is deliberately fairly cheap (via Arc),
// so this allows users to pass references to a client rather than explicitly
// cloning. This is partly for consistency with OnlineClient, which can be
// easily converted into an OfflineClient for ergonomics.
impl<'a, T: Config> From<&'a OfflineClient<T>> for OfflineClient<T> {
    fn from(c: &'a OfflineClient<T>) -> Self {
        c.clone()
    }
}
