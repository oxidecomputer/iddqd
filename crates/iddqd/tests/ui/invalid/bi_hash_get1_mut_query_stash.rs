// The `BiHashMap` form of `id_hash_get_mut_query_stash`.
use core::cell::Cell;
use core::hash::{Hash, Hasher};
use iddqd::{BiHashItem, BiHashMap, Equivalent, bi_upcast};

#[derive(Debug)]
struct Item {
    id: String,
    key2: u32,
    value: u32,
}

impl BiHashItem for Item {
    type K1<'a> = &'a str;
    type K2<'a> = u32;

    fn key1(&self) -> Self::K1<'_> {
        &self.id
    }

    fn key2(&self) -> Self::K2<'_> {
        self.key2
    }

    bi_upcast!();
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
    let mut map = BiHashMap::<Item>::new();
    map.insert_unique(Item { id: "foo".to_string(), key2: 10, value: 1 })
        .unwrap();

    let stash = Stash { probe: "foo", seen: Cell::new(None) };
    let mut item = map.get1_mut(&stash).unwrap();
    item.id.make_ascii_uppercase();
    item.value += 1;

    let leaked: &str = stash.seen.get().unwrap();
    println!("{leaked}");
}
