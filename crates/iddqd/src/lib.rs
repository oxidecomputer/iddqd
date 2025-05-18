// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Maps where the keys are part of the values.
//!
//! # Motivation
//!
//! Consider a typical key-value map where the keys and values are associated
//! with each other. For example:
//!
//! ```
//! use std::collections::HashMap;
//!
//! let map: HashMap<String, u32> = HashMap::new();
//! ```
//!
//! Now, it's common to associate the keys and values with each other. One way
//! to do this is to pass around tuples, for example `(&str, u32)`. But that's
//! inconvenient, so users may instead store structs where the value contains a
//! duplicate copy of the key.
//!
//! ```
//! use std::collections::HashMap;
//!
//! struct MyStruct {
//!     key: String,
//!     value: u32,
//! }
//!
//! let mut map: HashMap<String, MyStruct> = HashMap::new();
//!
//! map.insert("foo".to_string(), MyStruct { key: "foo".to_string(), value: 42 });
//! ```
//!
//! But there's nothing here which enforces any kind of consistency between the
//! key and the value:
//!
//! ```
//! # use std::collections::HashMap;
//! # struct MyStruct {
//! #     key: String,
//! #     value: u32,
//! # }
//! let mut map: HashMap<String, MyStruct> = HashMap::new();
//!
//! // This is allowed, but it violates internal consistency.
//! map.insert("foo".to_string(), MyStruct { key: "bar".to_string(), value: 42 });
//! ```
//!
//! That's where this crate comes in. It provides map types where the keys are
//! part of the values.
//!
//! **TODO**: show example here.

#![warn(missing_docs)]

pub mod errors;
pub mod id_btree_map;
#[doc(hidden)]
pub mod internal;
mod macros;
mod support;
pub mod tri_hash_map;

pub use id_btree_map::{
    imp::IdBTreeMap,
    trait_defs::{IdBTreeMapEntry, IdBTreeMapEntryMut},
};
pub use tri_hash_map::{imp::TriHashMap, trait_defs::TriHashMapEntry};
