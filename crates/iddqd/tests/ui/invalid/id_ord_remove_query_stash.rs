// The `IdOrdMap` form of `id_hash_remove_query_stash`, through `Comparable`.
use core::cell::Cell;
use core::cmp::Ordering;
use iddqd::{Comparable, Equivalent, IdOrdItem, IdOrdMap, id_upcast};

#[derive(Debug)]
struct Item {
    id: String,
}

impl IdOrdItem for Item {
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

impl<'a> Equivalent<&'a str> for Stash<'a> {
    fn equivalent(&self, key: &&'a str) -> bool {
        *key == self.probe
    }
}

impl<'a> Comparable<&'a str> for Stash<'a> {
    fn compare(&self, key: &&'a str) -> Ordering {
        self.seen.set(Some(*key));
        self.probe.cmp(*key)
    }
}

fn main() {
    let mut map = IdOrdMap::<Item>::new();
    map.insert_unique(Item { id: "foo".to_string() }).unwrap();

    let stash = Stash { probe: "foo", seen: Cell::new(None) };
    drop(map.remove(&stash));

    let leaked: &str = stash.seen.get().unwrap();
    println!("{leaked}");
}
