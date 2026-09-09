// A lookup type whose `Equivalent` impl keeps the map-owned key it is shown.
// `get_mut` must not accept it: with a `Q: Equivalent<T::Key<'a>>` bound at
// the mutable borrow's lifetime, `stash` would hold a `&'a str` into the item
// while `item` mutates it.
use core::cell::Cell;
use core::hash::{Hash, Hasher};
use iddqd::{Equivalent, IdHashItem, IdHashMap, id_upcast};

#[derive(Debug)]
struct Item {
    id: String,
    value: u32,
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
    map.insert_unique(Item { id: "foo".to_string(), value: 1 }).unwrap();

    let stash = Stash { probe: "foo", seen: Cell::new(None) };
    let mut item = map.get_mut(&stash).unwrap();
    item.id.make_ascii_uppercase();
    item.value += 1;

    let leaked: &str = stash.seen.get().unwrap();
    println!("{leaked}");
}
