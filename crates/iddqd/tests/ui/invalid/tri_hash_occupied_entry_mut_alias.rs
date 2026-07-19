use iddqd::{TriHashItem, tri_hash_map, tri_upcast};

#[derive(Debug)]
struct Item {
    key1: u32,
    key2: u32,
    key3: u32,
}

impl TriHashItem for Item {
    type K1<'a> = u32;
    type K2<'a> = u32;
    type K3<'a> = u32;

    fn key1(&self) -> Self::K1<'_> {
        self.key1
    }

    fn key2(&self) -> Self::K2<'_> {
        self.key2
    }

    fn key3(&self) -> Self::K3<'_> {
        self.key3
    }

    tri_upcast!();
}

fn main() {
    let mut map = tri_hash_map! {
        Item { key1: 0, key2: 10, key3: 20 },
    };

    let mut entry = match map.entry(0, 10, 99) {
        iddqd::tri_hash_map::Entry::Occupied(entry) => entry,
        iddqd::tri_hash_map::Entry::Vacant(_) => unreachable!(),
    };

    let mut refs = entry.get_mut();
    let _by_key1 = refs.by_key1().unwrap();
    let _by_key2 = refs.by_key2().unwrap();
}
