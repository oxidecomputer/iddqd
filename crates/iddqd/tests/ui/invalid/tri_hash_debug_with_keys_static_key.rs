// A `Debug` impl that exists only for `Key<'static>` must not be usable with
// `debug_with_keys` on a map that lives shorter than `'static`: the key
// handed to `Debug::fmt` borrows from the map.

use iddqd::{TriHashItem, TriHashMap, tri_upcast};
use std::fmt;

#[derive(PartialEq, Eq, Hash)]
struct Key<'a>(&'a str);

impl fmt::Debug for Key<'static> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Debug)]
struct Item {
    id: String,
}

impl TriHashItem for Item {
    type K1<'a> = Key<'a>;
    type K2<'a> = Key<'a>;
    type K3<'a> = Key<'a>;

    fn key1(&self) -> Self::K1<'_> {
        Key(&self.id)
    }

    fn key2(&self) -> Self::K2<'_> {
        Key(&self.id)
    }

    fn key3(&self) -> Self::K3<'_> {
        Key(&self.id)
    }

    tri_upcast!();
}

fn main() {
    let mut map = TriHashMap::new();
    map.insert_unique(Item { id: "foo".to_owned() }).unwrap();
    let _ = format!("{:?}", map.debug_with_keys());
}
