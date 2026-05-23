use iddqd::{IdHashItem, IdHashMap, id_upcast};

#[derive(Debug)]
struct Item {
    id: u32,
}

impl IdHashItem for Item {
    type Key<'a> = u32;

    fn key(&self) -> Self::Key<'_> {
        self.id
    }

    id_upcast!();
}

fn main() {
    let mut map = IdHashMap::<Item>::new();
    map.insert_unique(Item { id: 0 }).unwrap();

    let mut stashed = Vec::new();
    map.entry(0).and_modify(|item| {
        stashed.push(item);
    });

    stashed[0].id = 1;
}
