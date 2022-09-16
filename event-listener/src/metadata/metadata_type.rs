// Copyright 2019-2022 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use std::{
	collections::HashMap,
	convert::TryFrom,
	sync::Arc,
};

use frame_metadata::{
	META_RESERVED,
	RuntimeMetadata,
	RuntimeMetadataPrefixed,
	RuntimeMetadataV14,
};

use crate::metadata::metadata_utils::{get_storage_hash, NotFound};

use super::hash_cache::HashCache;

/// Metadata error originated from inspecting the internal representation of the runtime metadata.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MetadataError {
	/// Module is not in metadata.
	#[error("Pallet not found")]
	PalletNotFound,
	/// Call is not in metadata.
	#[error("Call not found")]
	CallNotFound,
	/// Event is not in metadata.
	#[error("Pallet {0}, Event {0} not found")]
	EventNotFound(u8, u8),
	/// Storage is not in metadata.
	#[error("Storage not found")]
	StorageNotFound,
	/// Constant is not in metadata.
	#[error("Constant not found")]
	ConstantNotFound,
}

// We hide the innards behind an Arc so that it's easy to clone and share.
#[derive(Debug)]
struct MetadataInner {
	metadata: RuntimeMetadataV14,
	events: HashMap<(u8, u8), EventMetadata>,
	cached_storage_hashes: HashCache,
}

/// A representation of the runtime metadata received from a node.
#[derive(Clone, Debug)]
pub struct Metadata {
	inner: Arc<MetadataInner>,
}

impl Metadata {
	/// Returns the metadata for the event at the given pallet and event indices.
	pub fn event(
		&self,
		pallet_index: u8,
		event_index: u8,
	) -> Result<&EventMetadata, MetadataError> {
		let event = self
			.inner
			.events
			.get(&(pallet_index, event_index))
			.ok_or(MetadataError::EventNotFound(pallet_index, event_index))?;
		Ok(event)
	}

	/// Return the runtime metadata.
	pub fn runtime_metadata(&self) -> &RuntimeMetadataV14 {
		&self.inner.metadata
	}

	/// Obtain the unique hash for a specific storage entry.
	pub fn storage_hash(
		&self,
		pallet: &str,
		storage: &str,
	) -> Result<[u8; 32], MetadataError> {
		self.inner
			.cached_storage_hashes
			.get_or_insert(pallet, storage, || {
				get_storage_hash(&self.inner.metadata, pallet, storage)
					.map_err(|e| {
						match e {
							NotFound::Pallet => {
								MetadataError::PalletNotFound
							}
							NotFound::Item => {
								MetadataError::StorageNotFound
							}
						}
					})
			})
	}
}

/// Metadata for specific events.
#[derive(Clone, Debug)]
pub struct EventMetadata {
	// The pallet name is shared across every event, so put it
	// behind an Arc to avoid lots of needless clones of it existing.
	pallet: Arc<str>,
	event: String,
	fields: Vec<(Option<String>, u32)>,
	docs: Vec<String>,
}

impl EventMetadata {
	/// Get the name of the pallet from which the event was emitted.
	pub fn pallet(&self) -> &str {
		&self.pallet
	}

	/// Get the name of the pallet event which was emitted.
	pub fn event(&self) -> &str {
		&self.event
	}

	/// The names and types of each field in the event.
	pub fn fields(&self) -> &[(Option<String>, u32)] {
		&self.fields
	}

	/// Documentation for this event.
	pub fn docs(&self) -> &[String] {
		&self.docs
	}
}

/// Error originated from converting a runtime metadata [RuntimeMetadataPrefixed] to
/// the internal [Metadata] representation.
#[derive(Debug, thiserror::Error)]
pub enum InvalidMetadataError {
	/// Invalid prefix
	#[error("Invalid prefix")]
	InvalidPrefix,
	/// Invalid version
	#[error("Invalid version")]
	InvalidVersion,
	/// Type missing from type registry
	#[error("Type {0} missing from type registry")]
	MissingType(u32),
	/// Type was not a variant/enum type
	#[error("Type {0} was not a variant/enum type")]
	TypeDefNotVariant(u32),
}

impl TryFrom<RuntimeMetadataPrefixed> for Metadata {
	type Error = InvalidMetadataError;

	fn try_from(metadata: RuntimeMetadataPrefixed) -> Result<Self, Self::Error> {
		if metadata.0 != META_RESERVED {
			return Err(InvalidMetadataError::InvalidPrefix);
		}
		let metadata = match metadata.1 {
			RuntimeMetadata::V14(meta) => meta,
			_ => return Err(InvalidMetadataError::InvalidVersion),
		};

		let get_type_def_variant = |type_id: u32| {
			let ty = metadata
				.types
				.resolve(type_id)
				.ok_or(InvalidMetadataError::MissingType(type_id))?;
			if let scale_info::TypeDef::Variant(var) = ty.type_def() {
				Ok(var)
			} else {
				Err(InvalidMetadataError::TypeDefNotVariant(type_id))
			}
		};

		let mut events = HashMap::<(u8, u8), EventMetadata>::new();
		for pallet in &metadata.pallets {
			if let Some(event) = &pallet.event {
				let pallet_name: Arc<str> = pallet.name.to_string().into();
				let event_type_id = event.ty.id();
				let event_variant = get_type_def_variant(event_type_id)?;
				for variant in event_variant.variants() {
					events.insert(
						(pallet.index, variant.index()),
						EventMetadata {
							pallet: pallet_name.clone(),
							event: variant.name().to_owned(),
							fields: variant
								.fields()
								.iter()
								.map(|f| (f.name().map(|n| n.to_owned()), f.ty().id()))
								.collect(),
							docs: variant.docs().to_vec(),
						},
					);
				}
			}
		}

		Ok(Metadata {
			inner: Arc::new(MetadataInner {
				metadata,
				events,
				cached_storage_hashes: Default::default(),
			}),
		})
	}
}

#[cfg(test)]
mod tests {
	use frame_metadata::{
		ExtrinsicMetadata,
		PalletStorageMetadata,
		StorageEntryModifier,
		StorageEntryType,
	};
	use scale_info::{
		meta_type,
		TypeInfo,
	};

	use super::*;

	fn load_metadata() -> Metadata {
		#[allow(dead_code)]
		#[allow(non_camel_case_types)]
		#[derive(TypeInfo)]
		enum Call {
			fill_block { param: u128 },
		}
		let storage = PalletStorageMetadata {
			prefix: "System",
			entries: vec![StorageEntryMetadata {
				name: "Account",
				modifier: StorageEntryModifier::Optional,
				ty: StorageEntryType::Plain(meta_type::<u32>()),
				default: vec![0],
				docs: vec![],
			}],
		};
		let constant = PalletConstantMetadata {
			name: "BlockWeights",
			ty: meta_type::<u32>(),
			value: vec![1, 2, 3],
			docs: vec![],
		};
		let pallet = frame_metadata::PalletMetadata {
			index: 0,
			name: "System",
			calls: Some(frame_metadata::PalletCallMetadata {
				ty: meta_type::<Call>(),
			}),
			storage: Some(storage),
			constants: vec![constant],
			event: None,
			error: None,
		};

		let metadata = RuntimeMetadataV14::new(
			vec![pallet],
			ExtrinsicMetadata {
				ty: meta_type::<()>(),
				version: 0,
				signed_extensions: vec![],
			},
			meta_type::<()>(),
		);
		let prefixed = RuntimeMetadataPrefixed::from(metadata);

		Metadata::try_from(prefixed)
			.expect("Cannot translate runtime metadata to internal Metadata")
	}

	#[test]
	fn metadata_inner_cache() {
		// Note: Dependency on test_runtime can be removed if complex metadata
		// is manually constructed.
		let metadata = load_metadata();

		let hash = metadata.metadata_hash(&["System"]);
		// Check inner caching.
		assert_eq!(metadata.inner.cached_metadata_hash.read().unwrap(), hash);

		// The cache `metadata.inner.cached_metadata_hash` is already populated from
		// the previous call. Therefore, changing the pallets argument must not
		// change the methods behavior.
		let hash_old = metadata.metadata_hash(&["no-pallet"]);
		assert_eq!(hash_old, hash);
	}

	#[test]
	fn metadata_call_inner_cache() {
		let metadata = load_metadata();

		let hash = metadata.call_hash("System", "fill_block");

		let mut call_number = 0;
		let hash_cached = metadata.inner.cached_call_hashes.get_or_insert(
			"System",
			"fill_block",
			|| -> Result<[u8; 32], MetadataError> {
				call_number += 1;
				Ok([0; 32])
			},
		);

		// Check function is never called (e.i, value fetched from cache).
		assert_eq!(call_number, 0);
		assert_eq!(hash.unwrap(), hash_cached.unwrap());
	}

	#[test]
	fn metadata_constant_inner_cache() {
		let metadata = load_metadata();

		let hash = metadata.constant_hash("System", "BlockWeights");

		let mut call_number = 0;
		let hash_cached = metadata.inner.cached_constant_hashes.get_or_insert(
			"System",
			"BlockWeights",
			|| -> Result<[u8; 32], MetadataError> {
				call_number += 1;
				Ok([0; 32])
			},
		);

		// Check function is never called (e.i, value fetched from cache).
		assert_eq!(call_number, 0);
		assert_eq!(hash.unwrap(), hash_cached.unwrap());
	}

	#[test]
	fn metadata_storage_inner_cache() {
		let metadata = load_metadata();
		let hash = metadata.storage_hash("System", "Account");

		let mut call_number = 0;
		let hash_cached = metadata.inner.cached_storage_hashes.get_or_insert(
			"System",
			"Account",
			|| -> Result<[u8; 32], MetadataError> {
				call_number += 1;
				Ok([0; 32])
			},
		);

		// Check function is never called (e.i, value fetched from cache).
		assert_eq!(call_number, 0);
		assert_eq!(hash.unwrap(), hash_cached.unwrap());
	}
}
