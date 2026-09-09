// The `IdOrdMap` form of `id_hash_upcast_key_stash`.
use core::cell::Cell;
use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use iddqd::{IdOrdItem, IdOrdMap};

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
        self.s == other.s
    }
}

impl<'a> Eq for StashKey<'a> {}

impl<'a> PartialOrd for StashKey<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for StashKey<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        if let Some(cell) = self.stash {
            cell.set(Some(other.s));
        }
        self.s.cmp(other.s)
    }
}

impl<'a> Hash for StashKey<'a> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.s.hash(h)
    }
}

impl IdOrdItem for Item {
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
    let mut map = IdOrdMap::<Item>::new();
    map.insert_unique(Item { id: "foo".to_string() }).unwrap();

    let cell = Cell::new(None);
    drop(map.remove(StashKey { s: "foo", stash: Some(&cell) }));

    let leaked: &str = cell.get().unwrap();
    println!("{leaked}");
}
