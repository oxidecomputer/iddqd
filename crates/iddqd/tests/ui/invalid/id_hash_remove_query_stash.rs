// Like `id_hash_get_mut_query_stash`, through `remove`: the stashed `&'a str`
// would point into the removed item's freed `String` buffer.
use core::cell::Cell;
use core::hash::{Hash, Hasher};
use iddqd::{Equivalent, IdHashItem, IdHashMap, id_upcast};

#[derive(Debug)]
struct Item {
    id: String,
}

impl IdHashItem for Item {
    type Key<'a> = &'a str;

    fn key(&self) -> Self::Key<'_> {
        &self.id
    }

    id_upcast!();
}

struct Stash<'a> {
    probe: &'static str,
    seen: Cell<Option<&'a str>>,
}

impl<'a> Hash for Stash<'a> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.probe.hash(h)
    }
}

impl<'a> Equivalent<&'a str> for Stash<'a> {
    fn equivalent(&self, key: &&'a str) -> bool {
        self.seen.set(Some(*key));
        *key == self.probe
    }
}

fn main() {
    let mut map = IdHashMap::<Item>::new();
    map.insert_unique(Item { id: "foo".to_string() }).unwrap();

    let stash = Stash { probe: "foo", seen: Cell::new(None) };
    drop(map.remove(&stash));

    let leaked: &str = stash.seen.get().unwrap();
    println!("{leaked}");
}
