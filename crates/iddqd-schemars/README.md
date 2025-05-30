<!-- cargo-sync-rdme title [[ -->
# iddqd-schemars
<!-- cargo-sync-rdme ]] -->
<!-- cargo-sync-rdme badge [[ -->
![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/iddqd-schemars.svg?)
[![crates.io](https://img.shields.io/crates/v/iddqd-schemars.svg?logo=rust)](https://crates.io/crates/iddqd-schemars)
[![docs.rs](https://img.shields.io/docsrs/iddqd-schemars.svg?logo=docs.rs)](https://docs.rs/iddqd-schemars)
[![Rust: ^1.81.0](https://img.shields.io/badge/rust-^1.81.0-93450a.svg?logo=rust)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field)
<!-- cargo-sync-rdme ]] -->
<!-- cargo-sync-rdme rustdoc [[ -->
JsonSchema implementations for iddqd map types using [schemars
0.8](https://docs.rs/schemars/0.8.22/schemars/index.html).

This crate provides JSON Schema generation support for the various map types
in the [iddqd](https://crates.io/crates/iddqd) crate.

All map types serialize as arrays of their values (matching their serde
serialization format), so the JSON schemas generated reflect this structure.

## Usage

Use the marker types in this crate with the `#[schemars(with = "Type")]`
attribute.

### Examples

````rust
use iddqd::{IdHashItem, IdHashMap, id_upcast};
use iddqd_schemars::IdHashMapSchema;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct User {
    name: String,
    age: u32,
}

impl IdHashItem for User {
    type Key<'a> = &'a str;

    fn key(&self) -> Self::Key<'_> {
        &self.name
    }

    id_upcast!();
}

#[derive(Serialize, Deserialize, JsonSchema)]
struct MyStruct {
    #[schemars(with = "IdHashMapSchema<User>")]
    users: IdHashMap<User>,
}

// Generate schema for MyStruct
let schema = schema_for!(MyStruct);
````

### `x-rust-type` and typify integration

The schemars in this crate produce an extra `x-rust-type` extension field
that can be used to integrate with [typify] by specifying `iddqd` in the
`crates` table. See the [`typify-types` example].

## Why is this its own crate?

If this were a feature in the iddqd crate, it wouldn’t be possible for
dependencies of schemars to use iddqd. Having these implementations live in
a separate crate works around this circular dependency limitation.

[typify]: https://crates.io/crates/typify
[`typify-types` example]: https://github.com/oxidecomputer/iddqd/blob/main/crates/iddqd-schemars/examples/typify-types.rs
<!-- cargo-sync-rdme ]] -->

## License

This project is available under the terms of either the [Apache 2.0 license](LICENSE-APACHE) or the [MIT
license](LICENSE-MIT).
