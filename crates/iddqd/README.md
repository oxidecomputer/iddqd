<!-- cargo-sync-rdme title [[ -->
# iddqd
<!-- cargo-sync-rdme ]] -->
<!-- cargo-sync-rdme badge [[ -->
![License: MPL-2.0](https://img.shields.io/crates/l/iddqd.svg?)
[![crates.io](https://img.shields.io/crates/v/iddqd.svg?logo=rust)](https://crates.io/crates/iddqd)
[![docs.rs](https://img.shields.io/docsrs/iddqd.svg?logo=docs.rs)](https://docs.rs/iddqd)
[![Rust: ^1.81.0](https://img.shields.io/badge/rust-^1.81.0-93450a.svg?logo=rust)](https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field)
<!-- cargo-sync-rdme ]] -->
<!-- cargo-sync-rdme rustdoc [[ -->
Maps where the keys are part of the values.

## Motivation

Consider a typical key-value map where the keys and values are associated
with each other. For example:

````rust
use std::collections::HashMap;

let map: HashMap<String, u32> = HashMap::new();
````

Now, it’s common to associate the keys and values with each other. One way
to do this is to pass around tuples, for example `(&str, u32)`. But that’s
inconvenient, so users may instead store structs where the value contains a
duplicate copy of the key.

````rust
use std::collections::HashMap;

struct MyStruct {
    key: String,
    value: u32,
}

let mut map: HashMap<String, MyStruct> = HashMap::new();

map.insert("foo".to_string(), MyStruct { key: "foo".to_string(), value: 42 });
````

But there’s nothing here which enforces any kind of consistency between the
key and the value:

````rust
let mut map: HashMap<String, MyStruct> = HashMap::new();

// This is allowed, but it violates internal consistency.
map.insert("foo".to_string(), MyStruct { key: "bar".to_string(), value: 42 });
````

That’s where this crate comes in. It provides map types where the keys are
part of the values.

**TODO**: show example here.
<!-- cargo-sync-rdme ]] -->

## License

This project is available under the terms of either the [Apache 2.0 license](LICENSE-APACHE) or the [MIT
license](LICENSE-MIT).
