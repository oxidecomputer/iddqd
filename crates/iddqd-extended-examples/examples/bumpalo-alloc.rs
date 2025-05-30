//! An example demonstrating use of the [`bumpalo`] crate to allocate against.
//!
//! Bumpalo and other custom allocators can be used with iddqd's hash map types.
//!
//! Requires the `allocator-api2` feature in iddqd.

use bumpalo::Bump;
use iddqd::{IdHashItem, IdHashMap, id_upcast};

/// These are the items we'll store in the `IdHashMap`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MyStruct<'bump> {
    // Because bumpalo doesn't run destructors on drop, we use strings and
    // vectors provided by the bumpalo crate. See https://docs.rs/bumpalo for
    // more.
    a: bumpalo::collections::String<'bump>,
    b: usize,
    c: bumpalo::collections::Vec<'bump, usize>,
}

/// The map will be indexed uniquely by (b, c). Note that this is a borrowed key
/// that can be constructed efficiently.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct MyKey<'a> {
    b: usize,
    c: &'a [usize],
}

impl<'bump> IdHashItem for MyStruct<'bump> {
    type Key<'a>
        = MyKey<'a>
    where
        Self: 'a;

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
        a: bumpalo::format!(in &bump, "Hello",),
        b: 42,
        c: bumpalo::vec![in &bump; 1, 2, 3],
    };
    map.insert_unique(v1.clone()).unwrap();

    let v2 = MyStruct {
        a: bumpalo::format!(in &bump, "World",),
        b: 42,
        c: bumpalo::vec![in &bump; 4, 5, 6],
    };
    map.insert_unique(v2).unwrap();

    // Retrieve an item from the map.
    let item = map.get(&MyKey { b: 42, c: &[4, 5, 6] });
    println!("retrieved {item:?}");
    assert_eq!(item, Some(&v1));

    // map cannot live longer than the bumpalo arena, thus ensuring memory
    // safety.
}
