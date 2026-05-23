use iddqd::{BiHashItem, BiHashMap, bi_upcast};

#[derive(Debug)]
struct Item {
    id: u32,
    key2: u32,
}

impl BiHashItem for Item {
    type K1<'a> = u32;
    type K2<'a> = u32;

    fn key1(&self) -> Self::K1<'_> {
        self.id
    }

    fn key2(&self) -> Self::K2<'_> {
        self.key2
    }

    bi_upcast!();
}

fn main() {
    let mut map = BiHashMap::<Item>::new();
    map.insert_unique(Item { id: 0, key2: 10 }).unwrap();

    let mut stashed = Vec::new();
    map.entry(0, 10).and_modify(|item| {
        stashed.push(item);
    });

    stashed[0].id = 1;
}
