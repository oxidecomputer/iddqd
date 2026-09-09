// A key type that carries interior-mutable storage typed at its own lifetime,
// whose `Eq` copies the map-owned key it is compared against into that
// storage. The mutable lookups pass the caller's key through `upcast_key`, and
// a type like this is invariant in `'a`, so `upcast_key` cannot be written for
// it. That is what keeps `eq` from smuggling a reference out of the lookup.
use core::cell::Cell;
use core::hash::{Hash, Hasher};
use iddqd::{IdHashItem, IdHashMap};

#[derive(Debug)]
struct Item {
    id: String,
}

struct StashKey<'a> {
    s: &'a str,
    stash: Option<&'a Cell<Option<&'a str>>>,
}

impl<'a> PartialEq for StashKey<'a> {
    fn eq(&self, other: &Self) -> bool {
        if let Some(cell) = self.stash {
            cell.set(Some(other.s));
        }
        self.s == other.s
    }
}

impl<'a> Eq for StashKey<'a> {}

impl<'a> Hash for StashKey<'a> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.s.hash(h)
    }
}

impl IdHashItem for Item {
    type Key<'a> = StashKey<'a>;

    fn key(&self) -> Self::Key<'_> {
        StashKey { s: &self.id, stash: None }
    }

    fn upcast_key<'short, 'long: 'short>(
        long: Self::Key<'long>,
    ) -> Self::Key<'short> {
        StashKey { s: long.s, stash: long.stash }
    }
}

fn main() {
    let mut map = IdHashMap::<Item>::new();
    map.insert_unique(Item { id: "foo".to_string() }).unwrap();

    let cell = Cell::new(None);
    drop(map.remove(StashKey { s: "foo", stash: Some(&cell) }));

    let leaked: &str = cell.get().unwrap();
    println!("{leaked}");
}
