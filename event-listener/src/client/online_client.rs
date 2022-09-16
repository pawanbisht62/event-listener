// Copyright 2019-2022 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use super::OfflineClientT;
use crate::{
    error::Error,
    events::EventsClient,
    rpc::{
        Rpc,
        RpcClientT,
        RuntimeVersion,
    },
    Config,
    Metadata,
};
use derivative::Derivative;
use futures::future;
use std::sync::Arc;
use parking_lot::RwLock;

/// A trait representing a client that can perform
/// online actions.
pub trait OnlineClientT<T: Config>: OfflineClientT<T> {
    /// Return an RPC client that can be used to communicate with a node.
    fn rpc(&self) -> &Rpc<T>;
}

/// A client that can be used to perform API calls (that is, either those
/// requiriing an [`OfflineClientT`] or those requiring an [`OnlineClientT`]).
#[derive(Derivative)]
#[derivative(Clone(bound = ""))]
pub struct OnlineClient<T: Config> {
    inner: Arc<RwLock<Inner>>,
    rpc: Rpc<T>,
}

#[derive(Derivative)]
#[derivative(Debug(bound = ""))]
struct Inner {
    runtime_version: RuntimeVersion,
    metadata: Metadata,
}

impl<T: Config> std::fmt::Debug for OnlineClient<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("rpc", &"RpcClient")
            .field("inner", &self.inner)
            .finish()
    }
}

// The default constructors assume Jsonrpsee.
#[cfg(feature = "jsonrpsee")]
impl<T: Config> OnlineClient<T> {
    /// Construct a new [`OnlineClient`] using default settings which
    /// point to a locally running node on `ws://127.0.0.1:9944`.
    pub async fn new() -> Result<OnlineClient<T>, Error> {
        let url = "ws://127.0.0.1:9944";
        OnlineClient::from_url(url).await
    }

    /// Construct a new [`OnlineClient`], providing a URL to connect to.
    pub async fn from_url(url: impl AsRef<str>) -> Result<OnlineClient<T>, Error> {
        let client = jsonrpsee_helpers::ws_client(url.as_ref())
            .await
            .map_err(|e| crate::error::RpcError(e.to_string()))?;
        OnlineClient::from_rpc_client(client).await
    }
}

impl<T: Config> OnlineClient<T> {
    /// Construct a new [`OnlineClient`] by providing an underlying [`RpcClientT`]
    /// implementation to drive the connection.
    pub async fn from_rpc_client<R: RpcClientT>(
        rpc_client: R,
    ) -> Result<OnlineClient<T>, Error> {
        let rpc = Rpc::new(rpc_client);

        let ( runtime_version, metadata) = future::join(
            rpc.runtime_version(None),
            rpc.metadata(),
        )
        .await;

        Ok(OnlineClient {
            inner: Arc::new(RwLock::new(Inner {
                runtime_version: runtime_version?,
                metadata: metadata?,
            })),
            rpc,
        })
    }

    /// Return the [`Metadata`] used in this client.
    pub fn metadata(&self) -> Metadata {
        let inner = self.inner.read();
        inner.metadata.clone()
    }

    /// Return the runtime version.
    pub fn runtime_version(&self) -> RuntimeVersion {
        let inner = self.inner.read();
        inner.runtime_version.clone()
    }

    /// Work with events.
    pub fn events(&self) -> EventsClient<T, Self> {
        <Self as OfflineClientT<T>>::events(self)
    }
}


impl<T: Config> OfflineClientT<T> for OnlineClient<T> {
    fn metadata(&self) -> Metadata {
        self.metadata()
    }
    fn runtime_version(&self) -> RuntimeVersion {
        self.runtime_version()
    }
}

impl<T: Config> OnlineClientT<T> for OnlineClient<T> {
    fn rpc(&self) -> &Rpc<T> {
        &self.rpc
    }
}

// helpers for a jsonrpsee specific OnlineClient.
#[cfg(feature = "jsonrpsee")]
mod jsonrpsee_helpers {
    pub use jsonrpsee::{
        client_transport::ws::{
            InvalidUri,
            Receiver,
            Sender,
            Uri,
            WsTransportClientBuilder,
        },
        core::{
            client::{
                Client,
                ClientBuilder,
            },
            Error,
        },
    };

    /// Build WS RPC client from URL
    pub async fn ws_client(url: &str) -> Result<Client, Error> {
        let (sender, receiver) = ws_transport(url).await?;
        Ok(ClientBuilder::default()
            .max_notifs_per_subscription(4096)
            .build_with_tokio(sender, receiver))
    }

    async fn ws_transport(url: &str) -> Result<(Sender, Receiver), Error> {
        let url: Uri = url
            .parse()
            .map_err(|e: InvalidUri| Error::Transport(e.into()))?;
        WsTransportClientBuilder::default()
            .build(url)
            .await
            .map_err(|e| Error::Transport(e.into()))
    }
}
