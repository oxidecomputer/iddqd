//! An example demonstrating use of the [`bumpalo`] crate to allocate against.
//!
//! Bumpalo and other custom allocators can be used with iddqd's hash map types.
//!
//! Requires the `allocator-api2` feature in iddqd.

use bumpalo::Bump;
use iddqd::{IdHashItem, IdHashMap, id_upcast};
use std::path::{Path, PathBuf};

/// These are the items we'll store in the `IdHashMap`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MyStruct {
    a: String,
    b: usize,
    c: PathBuf,
    d: Vec<usize>,
}

/// The map will be indexed uniquely by (b, c, d). Note that this is a
/// borrowed key that can be constructed efficiently.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct MyKey<'a> {
    b: usize,
    c: &'a Path,
}

impl IdHashItem for MyStruct {
    type Key<'a> = MyKey<'a>;

    fn key(&self) -> Self::Key<'_> {
        MyKey { b: self.b, c: &self.c }
    }

    id_upcast!();
}

fn main() {
    // Create a new bumpalo arena.
    let bump = Bump::new();

    // Create a new IdHashMap using the bumpalo allocator.
    let mut map = IdHashMap::new_in(&bump);

    // Insert some items into the map.
    let v1 = MyStruct {
        a: "Hello".to_string(),
        b: 42,
        c: PathBuf::from("/path/to/file"),
        d: vec![1, 2, 3],
    };
    map.insert_unique(v1.clone()).unwrap();

    let v2 = MyStruct {
        a: "World".to_string(),
        b: 42,
        c: PathBuf::from("/path/to/another/file"),
        d: vec![4, 5, 6],
    };
    map.insert_unique(v2).unwrap();

    // Retrieve an item from the map.
    let item = map.get(&MyKey { b: 42, c: Path::new("/path/to/file") });
    println!("retrieved {item:?}");
    assert_eq!(item, Some(&v1));

    // map cannot live longer than the bumpalo arena, thus ensuring memory
    // safety.
}
