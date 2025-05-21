<!-- cargo-sync-rdme title [[ -->
# iddqd
<!-- cargo-sync-rdme ]] -->
<!-- cargo-sync-rdme badge [[ -->
![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/iddqd.svg?)
[![crates.io](https://img.shields.io/crates/v/iddqd.svg?logo=rust)](https://crates.io/crates/iddqd)
[![docs.rs](https://img.shields.io/docsrs/iddqd.svg?logo=docs.rs)](https://docs.rs/iddqd)
[![Rust: ^1.81.0](https://img.shields.io/badge/rust-^1.81.0-93450a.svg?logo=rust)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field)
<!-- cargo-sync-rdme ]] -->
<!-- cargo-sync-rdme rustdoc [[ -->
Maps where keys are borrowed from values.

This crate consists of several map types, collectively called **ID maps**:

* [`IdOrdMap`](https://docs.rs/iddqd/0.1.0/iddqd/id_ord_map/imp/struct.IdOrdMap.html): A B-Tree based map where keys are borrowed from values.
* [`IdHashMap`](https://docs.rs/iddqd/0.1.0/iddqd/id_hash_map/imp/struct.IdHashMap.html): A hash map where keys are borrowed from values.
* [`BiHashMap`](https://docs.rs/iddqd/0.1.0/iddqd/bi_hash_map/imp/struct.BiHashMap.html): A bijective (1:1) hash map with two keys, borrowed from values.
* [`TriHashMap`](https://docs.rs/iddqd/0.1.0/iddqd/tri_hash_map/imp/struct.TriHashMap.html): A trijective (1:1:1) hash map with three keys, borrowed from values.

## Usage

* Pick your ID map type.
* Depending on the ID map type, implement [`IdOrdItem`](https://docs.rs/iddqd/0.1.0/iddqd/id_ord_map/trait_defs/trait.IdOrdItem.html), [`IdHashItem`](https://docs.rs/iddqd/0.1.0/iddqd/id_hash_map/trait_defs/trait.IdHashItem.html), [`BiHashItem`](https://docs.rs/iddqd/0.1.0/iddqd/bi_hash_map/trait_defs/trait.BiHashItem.html), or
  [`TriHashItem`](https://docs.rs/iddqd/0.1.0/iddqd/tri_hash_map/trait_defs/trait.TriHashItem.html) for your value type.
* Store values in the ID map type.

### Features

This crate was built out a practical need for map types, and addresses
issues encountered using Rust’s default map types in practice at Oxide.

* Keys are retrieved from values, not stored separately from them. Separate
  storage has been a recurring pain point in our codebases: if keys are
  duplicated within values, it’s proven to be hard to maintain consistency
  between keys and values. This crate addresses that need.
* Keys may be borrowed from values, which allows for more flexible
  implementations. (They don’t have to be borrowed, but they can be.)
* There’s no `insert` method; insertion must be through either
  `insert_overwrite` or `insert_unique`. You must pick an insertion
  behavior.
* The serde implementations reject duplicate keys.

We’ve also sometimes needed to index a set of data by more than one key, or
perhaps map one key to another. For that purpose, this crate provides
[`BiHashMap`](https://docs.rs/iddqd/0.1.0/iddqd/bi_hash_map/imp/struct.BiHashMap.html) and [`TriHashMap`](https://docs.rs/iddqd/0.1.0/iddqd/tri_hash_map/imp/struct.TriHashMap.html).

* [`BiHashMap`](https://docs.rs/iddqd/0.1.0/iddqd/bi_hash_map/imp/struct.BiHashMap.html) has two keys, and provides a bijection (1:1 relationship)
  between the keys.
* [`TriHashMap`](https://docs.rs/iddqd/0.1.0/iddqd/tri_hash_map/imp/struct.TriHashMap.html) has three keys, and provides a trijection (1:1:1 relationship)
  between the keys.

Due to their general structure, maps can have arbitrary value data
associated with them as well.

### Examples

An example for [`IdOrdMap`](https://docs.rs/iddqd/0.1.0/iddqd/id_ord_map/imp/struct.IdOrdMap.html):

````rust
use iddqd::{IdOrdMap, IdOrdItem, id_upcast};

#[derive(Debug)]
struct User {
    name: String,
    age: u8,
}

// Implement IdOrdItem so the map knows how to get the key from the value.
impl IdOrdItem for User {
    // The key type can borrow from the value.
    type Key<'a> = &'a str;

    fn key(&self) -> Self::Key<'_> {
        &self.name
    }

    id_upcast!();
}

let mut users = IdOrdMap::<User>::new();

// You must pick an insertion behavior. insert_unique returns an error if
// the key already exists.
users.insert_unique(User { name: "Alice".to_string(), age: 30 }).unwrap();
users.insert_unique(User { name: "Bob".to_string(), age: 35 }).unwrap();

// Lookup by name:
assert_eq!(users.get("Alice").unwrap().age, 30);
assert_eq!(users.get("Bob").unwrap().age, 35);

// Iterate over users:
for user in &users {
    println!("User {}: {}", user.name, user.age);
}
````

An example for [`IdHashMap`](https://docs.rs/iddqd/0.1.0/iddqd/id_hash_map/imp/struct.IdHashMap.html), showing complex borrowed keys.

````rust
use iddqd::{IdHashMap, IdHashItem, id_upcast};

#[derive(Debug)]
struct Artifact {
    name: String,
    version: String,
    data: Vec<u8>,
}

// The key type is a borrowed form of the name and version. It needs to
// implement `Hash + Eq`.
#[derive(Hash, PartialEq, Eq)]
struct ArtifactKey<'a> {
    name: &'a str,
    version: &'a str,
}

impl IdHashItem for Artifact {
    // The key type can borrow from the value.
    type Key<'a> = ArtifactKey<'a>;

    fn key(&self) -> Self::Key<'_> {
        ArtifactKey {
            name: &self.name,
            version: &self.version,
        }
    }

    id_upcast!();
}

let mut artifacts = IdHashMap::<Artifact>::new();

// Add artifacts to the map.
artifacts.insert_unique(Artifact {
    name: "artifact1".to_owned(),
    version: "1.0".to_owned(),
    data: b"data1".to_vec(),
})
.unwrap();
artifacts.insert_unique(Artifact {
    name: "artifact2".to_owned(),
    version: "1.0".to_owned(),
    data: b"data2".to_vec(),
})
.unwrap();

// Look up artifacts by name and version.
assert_eq!(
    artifacts
        .get(&ArtifactKey { name: "artifact1", version: "1.0" })
        .unwrap()
        .data,
    b"data1",
);
````

## No-std compatibility

Most of this crate is no-std compatible, though [`alloc`](https://doc.rust-lang.org/nightly/alloc/index.html) is required.

The [`IdOrdMap`](https://docs.rs/iddqd/0.1.0/iddqd/id_ord_map/imp/struct.IdOrdMap.html) type is not currently no-std compatible due to its use of a
thread-local. This thread-local is just a way to work around a limitation in
std’s `BTreeMap` API, though. Either a custom B-Tree implementation, or a
platform-specific notion of thread locals, would suffice to make
[`IdOrdMap`](https://docs.rs/iddqd/0.1.0/iddqd/id_ord_map/imp/struct.IdOrdMap.html) no-std compatible.

## Optional features

* `serde`: Enables serde support for all ID map types. *Not enabled by default.*
* `std`: Enables std support. *Enabled by default.*

## Related work

* [`bimap`](https://docs.rs/bimap) provides a bijective map, but does not
  have a way to associate arbitrary values with each pair of keys. However, it
  does support an ordered map type without the need for std.

## Minimum supported Rust version (MSRV)

This crate’s MSRV is **Rust 1.81**. In general we aim for 6 months of Rust
compatibility.
<!-- cargo-sync-rdme ]] -->

## License

This project is available under the terms of either the [Apache 2.0 license](LICENSE-APACHE) or the [MIT
license](LICENSE-MIT).

Portions adapted from [The Rust Programming Language](https://github.com/rust-lang/rust) and used under the MIT and Apache 2.0 licenses. The Rust Programming Language is (c) The Rust Project Contributors.
