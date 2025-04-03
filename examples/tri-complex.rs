// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! An example demonstrating `TriHashMap` use with complex borrowed keys.

use id_map::{tri_upcasts, TriHashMap, TriHashMapEntry};
use std::path::{Path, PathBuf};

/// These are the entries we'll store in the `TriHashMap`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MyStruct {
    a: String,
    b: usize,
    c: PathBuf,
    d: Vec<usize>,
}

/// The map will be indexed uniquely by (usize, &Path). Note that this is a
/// borrowed key that can be constructed efficiently.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct MyKey1<'a> {
    b: usize,
    c: &'a Path,
}

/// The map will also be indexed uniquely by (&Path, &[usize]).
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct MyKey2<'a> {
    c: &'a Path,
    d: &'a [usize],
}

impl TriHashMapEntry for MyStruct {
    type K1<'a> = MyKey1<'a>;
    type K2<'a> = MyKey2<'a>;
    // And finally, the map will be indexed uniquely by the `a` field, i.e.
    // String. (This could also be a borrowed key like `&'a str`, but we're
    // using String for this example to demonstrate the use of the `Borrow`
    // trait below.)
    type K3<'a> = String;

    fn key1(&self) -> Self::K1<'_> {
        MyKey1 { b: self.b, c: &self.c }
    }

    fn key2(&self) -> Self::K2<'_> {
        MyKey2 { c: &self.c, d: &self.d }
    }

    fn key3(&self) -> Self::K3<'_> {
        self.a.clone()
    }

    tri_upcasts!();
}

fn main() {
    // Make a `TriHashMap` with the keys we defined above.
    let mut map = TriHashMap::new();

    let entry = MyStruct {
        a: "example".to_owned(),
        b: 20,
        c: PathBuf::from("/"),
        d: Vec::new(),
    };

    // Add an entry to the map.
    map.insert_no_dups(entry.clone()).unwrap();

    // This entry will conflict with the previous one due to the `a` field
    // matching.
    map.insert_no_dups(MyStruct {
        a: "example".to_owned(),
        b: 30,
        c: PathBuf::from("/xyz"),
        d: vec![0],
    })
    .unwrap_err();

    // Lookups can happen based on any of the keys. For example, we can look up
    // an entry by the first key.
    assert_eq!(map.get1(&MyKey1 { b: 20, c: Path::new("/") }), Some(&entry));

    // We can also look up an entry by anything that implements `Borrow`. For
    // example, &str for the third key.
    assert_eq!(map.get3("example"), Some(&entry));
}
