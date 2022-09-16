// Copyright 2019-2022 Parity Technologies (UK) Ltd.
// This file is dual-licensed as Apache-2.0 or GPL-3.0.
// see LICENSE for license details.

use frame_metadata::{
	RuntimeMetadataV14,
	StorageEntryMetadata,
	StorageEntryType,
};
use scale_info::{
	form::PortableForm,
	Field,
	PortableRegistry,
	TypeDef,
	Variant,
};
use std::collections::HashSet;

/// Internal byte representation for various metadata types utilized for
/// generating deterministic hashes between different rust versions.
#[repr(u8)]
enum TypeBeingHashed {
	Composite,
	Variant,
	Sequence,
	Array,
	Tuple,
	Primitive,
	Compact,
	BitSequence,
}

/// Hashing function utilized internally.
fn hash(bytes: &[u8]) -> [u8; 32] {
	sp_core::hashing::twox_256(bytes)
}

/// XOR two hashes together. If we have two pseudorandom hashes, then this will
/// lead to another pseudorandom value. If there is potentially some pattern to
/// the hashes we are xoring (eg we might be xoring the same hashes a few times),
/// prefer `hash_hashes` to give us stronger pseudorandomness guarantees.
fn xor(a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
	let mut out = [0u8; 32];
	for (idx, (a, b)) in a.into_iter().zip(b).enumerate() {
		out[idx] = a ^ b;
	}
	out
}

/// Combine two hashes or hash-like sets of bytes together into a single hash.
/// `xor` is OK for one-off combinations of bytes, but if we are merging
/// potentially identical hashes, this is a safer way to ensure the result is
/// unique.
fn hash_hashes(a: [u8; 32], b: [u8; 32]) -> [u8; 32] {
	let mut out = [0u8; 32 * 2];
	for (idx, byte) in a.into_iter().chain(b).enumerate() {
		out[idx] = byte;
	}
	hash(&out)
}

/// Obtain the hash representation of a `scale_info::Field`.
fn get_field_hash(
	registry: &PortableRegistry,
	field: &Field<PortableForm>,
	visited_ids: &mut HashSet<u32>,
) -> [u8; 32] {
	let mut bytes = get_type_hash(registry, field.ty().id(), visited_ids);

	// XOR name and field name with the type hash if they exist
	if let Some(name) = field.name() {
		bytes = xor(bytes, hash(name.as_bytes()));
	}

	bytes
}

/// Obtain the hash representation of a `scale_info::Variant`.
fn get_variant_hash(
	registry: &PortableRegistry,
	var: &Variant<PortableForm>,
	visited_ids: &mut HashSet<u32>,
) -> [u8; 32] {
	// Merge our hashes of the name and each field together using xor.
	let mut bytes = hash(var.name().as_bytes());
	for field in var.fields() {
		bytes = hash_hashes(bytes, get_field_hash(registry, field, visited_ids))
	}

	bytes
}

/// Obtain the hash representation of a `scale_info::TypeDef`.
fn get_type_def_hash(
	registry: &PortableRegistry,
	ty_def: &TypeDef<PortableForm>,
	visited_ids: &mut HashSet<u32>,
) -> [u8; 32] {
	match ty_def {
		TypeDef::Composite(composite) => {
			let mut bytes = hash(&[TypeBeingHashed::Composite as u8]);
			for field in composite.fields() {
				bytes = hash_hashes(bytes, get_field_hash(registry, field, visited_ids));
			}
			bytes
		}
		TypeDef::Variant(variant) => {
			let mut bytes = hash(&[TypeBeingHashed::Variant as u8]);
			for var in variant.variants().iter() {
				bytes = hash_hashes(bytes, get_variant_hash(registry, var, visited_ids));
			}
			bytes
		}
		TypeDef::Sequence(sequence) => {
			let bytes = hash(&[TypeBeingHashed::Sequence as u8]);
			xor(
				bytes,
				get_type_hash(registry, sequence.type_param().id(), visited_ids),
			)
		}
		TypeDef::Array(array) => {
			// Take length into account; different length must lead to different hash.
			let len_bytes = array.len().to_be_bytes();
			let bytes = hash(&[
				TypeBeingHashed::Array as u8,
				len_bytes[0],
				len_bytes[1],
				len_bytes[2],
				len_bytes[3],
			]);
			xor(
				bytes,
				get_type_hash(registry, array.type_param().id(), visited_ids),
			)
		}
		TypeDef::Tuple(tuple) => {
			let mut bytes = hash(&[TypeBeingHashed::Tuple as u8]);
			for field in tuple.fields() {
				bytes =
					hash_hashes(bytes, get_type_hash(registry, field.id(), visited_ids));
			}
			bytes
		}
		TypeDef::Primitive(primitive) => {
			// Cloning the 'primitive' type should essentially be a copy.
			hash(&[TypeBeingHashed::Primitive as u8, primitive.clone() as u8])
		}
		TypeDef::Compact(compact) => {
			let bytes = hash(&[TypeBeingHashed::Compact as u8]);
			xor(
				bytes,
				get_type_hash(registry, compact.type_param().id(), visited_ids),
			)
		}
		TypeDef::BitSequence(bitseq) => {
			let mut bytes = hash(&[TypeBeingHashed::BitSequence as u8]);
			bytes = xor(
				bytes,
				get_type_hash(registry, bitseq.bit_order_type().id(), visited_ids),
			);
			bytes = xor(
				bytes,
				get_type_hash(registry, bitseq.bit_store_type().id(), visited_ids),
			);
			bytes
		}
	}
}

/// Obtain the hash representation of a `scale_info::Type` identified by id.
fn get_type_hash(
	registry: &PortableRegistry,
	id: u32,
	visited_ids: &mut HashSet<u32>,
) -> [u8; 32] {
	// Guard against recursive types and return a fixed arbitrary hash
	if !visited_ids.insert(id) {
		return hash(&[123u8])
	}

	let ty = registry.resolve(id).unwrap();
	get_type_def_hash(registry, ty.type_def(), visited_ids)
}

/// Get the hash corresponding to a single storage entry.
fn get_storage_entry_hash(
	registry: &PortableRegistry,
	entry: &StorageEntryMetadata<PortableForm>,
	visited_ids: &mut HashSet<u32>,
) -> [u8; 32] {
	let mut bytes = hash(entry.name.as_bytes());
	// Cloning 'entry.modifier' should essentially be a copy.
	bytes = xor(bytes, hash(&[entry.modifier.clone() as u8]));
	bytes = xor(bytes, hash(&entry.default));

	match &entry.ty {
		StorageEntryType::Plain(ty) => {
			bytes = xor(bytes, get_type_hash(registry, ty.id(), visited_ids));
		}
		StorageEntryType::Map {
			hashers,
			key,
			value,
		} => {
			for hasher in hashers {
				// Cloning the hasher should essentially be a copy.
				bytes = hash_hashes(bytes, [hasher.clone() as u8; 32]);
			}
			bytes = xor(bytes, get_type_hash(registry, key.id(), visited_ids));
			bytes = xor(bytes, get_type_hash(registry, value.id(), visited_ids));
		}
	}

	bytes
}

/// Obtain the hash for a specific storage item, or an error if it's not found.
pub fn get_storage_hash(
	metadata: &RuntimeMetadataV14,
	pallet_name: &str,
	storage_name: &str,
) -> Result<[u8; 32], NotFound> {
	let pallet = metadata
		.pallets
		.iter()
		.find(|p| p.name == pallet_name)
		.ok_or(NotFound::Pallet)?;

	let storage = pallet.storage.as_ref().ok_or(NotFound::Item)?;

	let entry = storage
		.entries
		.iter()
		.find(|s| s.name == storage_name)
		.ok_or(NotFound::Item)?;

	let hash = get_storage_entry_hash(&metadata.types, entry, &mut HashSet::new());
	Ok(hash)
}

/// An error returned if we attempt to get the hash for a specific call, constant
/// or storage item that doesn't exist.
#[derive(Clone, Debug)]
pub enum NotFound {
	Pallet,
	Item,
}
