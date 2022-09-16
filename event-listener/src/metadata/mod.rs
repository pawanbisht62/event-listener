// Copyright 2019-2022 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

//! Types representing the metadata obtained from a node.

mod hash_cache;
mod metadata_type;
mod metadata_utils;

pub use metadata_type::{
    EventMetadata,
    InvalidMetadataError,
    Metadata,
    MetadataError,
};
