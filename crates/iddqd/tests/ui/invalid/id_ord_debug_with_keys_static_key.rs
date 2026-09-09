// A `Debug` impl that exists only for `Key<'static>` must not be usable with
// `debug_with_keys` on a map that lives shorter than `'static`: the key
// handed to `Debug::fmt` borrows from the map.

use iddqd::{IdOrdItem, IdOrdMap, id_upcast};
use std::fmt;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl IdOrdItem for Item {
    type Key<'a> = Key<'a>;

    fn key(&self) -> Self::Key<'_> {
        Key(&self.id)
    }

    id_upcast!();
}

fn main() {
    let mut map = IdOrdMap::new();
    map.insert_unique(Item { id: "foo".to_owned() }).unwrap();
    let _ = format!("{:?}", map.debug_with_keys());
}
