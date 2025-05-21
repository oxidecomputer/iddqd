//! Maps where keys are borrowed from values.
//!
//! This crate consists of several map types, collectively called **ID maps**:
//!
//! - [`IdOrdMap`]: A B-Tree based map where keys are borrowed from values.
//! - [`IdHashMap`]: A hash map where keys are borrowed from values.
//! - [`BiHashMap`]: A hash map with two keys, borrowed from values.
//! - [`TriHashMap`]: A hash map with three keys, borrowed from values.
//!
//! # Usage
//!
//! * Pick your ID map type.
//! * Depending on the ID map type, implement [`IdOrdItem`], [`IdHashItem`], [`BiHashItem`], or
//!   [`TriHashItem`] for your value type.
//! * Store values in the ID map type.
//!
//! ## Features
//!
//! This crate was built out a practical need for map types, and addresses
//! issues encountered using Rust's default map types in practice at Oxide.
//!
//! * Keys are retrieved from values, not stored separately from them. Separate
//!   storage has been a recurring pain point in our codebases: if keys are
//!   duplicated within values, it's proven to be hard to maintain consistency
//!   between keys and values. This crate addresses that need.
//! * Keys may be borrowed from values, which allows for more flexible
//!   implementations. (They don't have to be borrowed, but they can be.)
//! * There's no `insert` method; insertion must be through either
//!   `insert_override` or `insert_unique`. You must pick an insertion
//!   behavior.
//! * The serde implementations reject duplicate keys.
//!
//! ## Examples
//!
//! An example for [`IdOrdMap`]:
//!
//! ```
//! use iddqd::{IdOrdMap, IdOrdItem, id_upcast};
//!
//! #[derive(Debug)]
//! struct User {
//!     name: String,
//!     age: u8,
//! }
//!
//! // Implement IdOrdItem so the map knows how to get the key from the value.
//! impl IdOrdItem for User {
//!     // The key type can borrow from the value.
//!     type Key<'a> = &'a str;
//!
//!     fn key(&self) -> Self::Key<'_> {
//!         &self.name
//!     }
//!
//!     id_upcast!();
//! }
//!
//! let mut users = IdOrdMap::<User>::new();
//!
//! // You must pick an insertion behavior. insert_unique returns an error if
//! // the key already exists.
//! users.insert_unique(User { name: "Alice".to_string(), age: 30 }).unwrap();
//! users.insert_unique(User { name: "Bob".to_string(), age: 35 }).unwrap();
//!
//! // Lookup by name:
//! assert_eq!(users.get("Alice").unwrap().age, 30);
//! assert_eq!(users.get("Bob").unwrap().age, 35);
//!
//! // Iterate over users:
//! for user in &users {
//!     println!("User {}: {}", user.name, user.age);
//! }
//! ```
//!
//! An example for [`IdHashMap`], showing complex borrowed keys.
//!
//! ```
//! use iddqd::{IdHashMap, IdHashItem, id_upcast};
//!
//! #[derive(Debug)]
//! struct Artifact {
//!     name: String,
//!     version: String,
//!     data: Vec<u8>,
//! }
//!
//! // The key type is a borrowed form of the name and version. It needs to
//! // implement `Hash + Eq`.
//! #[derive(Hash, PartialEq, Eq)]
//! struct ArtifactKey<'a> {
//!     name: &'a str,
//!     version: &'a str,
//! }
//!
//! impl IdHashItem for Artifact {
//!     // The key type can borrow from the value.
//!     type Key<'a> = ArtifactKey<'a>;
//!
//!     fn key(&self) -> Self::Key<'_> {
//!         ArtifactKey {
//!             name: &self.name,
//!             version: &self.version,
//!         }
//!     }
//!
//!     id_upcast!();
//! }
//!
//! let mut artifacts = IdHashMap::<Artifact>::new();
//!
//! // Add artifacts to the map.
//! artifacts.insert_unique(Artifact {
//!     name: "artifact1".to_owned(),
//!     version: "1.0".to_owned(),
//!     data: b"data1".to_vec(),
//! }).unwrap();
//! artifacts.insert_unique(Artifact {
//!     name: "artifact2".to_owned(),
//!     version: "1.0".to_owned(),
//!     data: b"data2".to_vec(),
//! }).unwrap();
//!
//! // Look up artifacts by name and version.
//! assert_eq!(
//!     artifacts
//!         .get(&ArtifactKey { name: "artifact1", version: "1.0" })
//!         .unwrap()
//!         .data,
//!     b"data1",
//! );
//! ```
//!
//! # Minimum supported Rust version (MSRV)
//!
//! This crate's MSRV is **Rust 1.86**. In general we aim for 6 months of Rust
//! compatibility, but this crate requires a feature new to Rust 1.86.
//!
//! # Optional features
//!
//! - `serde`: Enables serde support for all ID map types. *Not enabled by default.*

#![cfg_attr(doc_cfg, feature(doc_auto_cfg))]
#![warn(missing_docs)]

pub mod bi_hash_map;
pub mod errors;
pub mod id_hash_map;
pub mod id_ord_map;
#[doc(hidden)]
pub mod internal;
mod macros;
mod support;
pub mod tri_hash_map;

pub use bi_hash_map::{imp::BiHashMap, trait_defs::BiHashItem};
pub use id_hash_map::{imp::IdHashMap, trait_defs::IdHashItem};
pub use id_ord_map::{imp::IdOrdMap, trait_defs::IdOrdItem};
pub use tri_hash_map::{imp::TriHashMap, trait_defs::TriHashItem};
