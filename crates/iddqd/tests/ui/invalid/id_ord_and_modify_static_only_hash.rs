// A `Hash` impl that exists only for `Key<'static>` must not be usable with
// `and_modify`: the entry can remove the item afterwards, so a key handed to
// `Hash::hash` for `'static` would outlive the item. Leaking the map is what
// makes `'a = 'static` reachable.

use iddqd::{IdOrdItem, IdOrdMap, id_upcast};
use std::hash::{Hash, Hasher};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Key<'a>(&'a str);

impl Hash for Key<'static> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
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
    let map: &'static mut IdOrdMap<Item> = Box::leak(Box::new(IdOrdMap::new()));
    map.insert_unique(Item { id: "foo".to_owned() }).unwrap();
    let entry = map.entry(Key("foo")).and_modify(|_| {});
    if let iddqd::id_ord_map::Entry::Occupied(entry) = entry {
        drop(entry.remove());
    }
}
