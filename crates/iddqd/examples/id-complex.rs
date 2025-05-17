// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! An example demonstrating `IdBTreeMap` use with complex borrowed keys.

use iddqd::{id_btree_map::Entry, id_upcast, IdBTreeMap, IdBTreeMapEntry};
use std::path::{Path, PathBuf};

/// These are the entries we'll store in the `IdBTreeMap`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MyStruct {
    a: String,
    b: usize,
    c: PathBuf,
    d: Vec<usize>,
}

/// The map will be indexed uniquely by (b, c, d). Note that this is a
/// borrowed key that can be constructed efficiently.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MyKey<'a> {
    b: usize,
    c: &'a Path,
    d: &'a [usize],
}

impl IdBTreeMapEntry for MyStruct {
    type Key<'a> = MyKey<'a>;

    fn key(&self) -> Self::Key<'_> {
        MyKey { b: self.b, c: &self.c, d: &self.d }
    }

    id_upcast!();
}

fn main() {
    // Make a `TriHashMap` with the keys we defined above.
    let mut map = IdBTreeMap::new();

    let entry = MyStruct {
        a: "example".to_owned(),
        b: 20,
        c: PathBuf::from("/"),
        d: Vec::new(),
    };

    // Add an entry to the map.
    map.insert_unique(entry.clone()).unwrap();

    // This entry will conflict with the previous one due to b, c and d
    // matching.
    map.insert_unique(MyStruct {
        a: "something-else".to_owned(),
        b: 20,
        c: PathBuf::from("/"),
        d: Vec::new(),
    })
    .unwrap_err();

    // Add another entry to the map. Note that this entry has the same c and d
    // but a different b.
    let entry2 = MyStruct {
        a: "example".to_owned(),
        b: 10,
        c: PathBuf::from("/"),
        d: Vec::new(),
    };
    map.insert_unique(entry2.clone()).unwrap();

    // Lookups can happen based on a borrowed key. For example:
    assert_eq!(
        map.get(&MyKey { b: 20, c: Path::new("/"), d: &[] }),
        Some(&entry)
    );

    // While iterating over the map, entries will be sorted by their key.
    for entry in map.iter() {
        println!("{:?}", entry);
    }

    let entry3 = MyStruct {
        a: "example".to_owned(),
        b: 20,
        c: PathBuf::from("/"),
        d: vec![1, 2, 3],
    };

    for item in [entry, entry2, entry3] {
        let entry = map.entry(item.key());
        match entry {
            Entry::Occupied(entry) => {
                // We can get the entry's value.
                let value = entry.get();
                println!("occupied: {:?}", value);
            }
            Entry::Vacant(entry) => {
                // We can insert a new value.
                let value = entry.insert(item);
                println!("inserted: {:?}", value);
            }
        }
    }
}
