// Item types that carry a lifetime cannot call `retain` or `and_modify`: the
// `for<'k> T::Key<'k>: Hash` bound those methods need implies `T: 'static`
// under the current compiler. Every other method, including `get_mut`, still
// works for them.

use iddqd::{IdOrdItem, IdOrdMap, id_upcast};

#[derive(Debug)]
struct BorrowedItem<'a> {
    id: &'a str,
    value: u32,
}

impl<'a> IdOrdItem for BorrowedItem<'a> {
    type Key<'k>
        = &'a str
    where
        Self: 'k;

    fn key(&self) -> Self::Key<'_> {
        self.id
    }

    id_upcast!();
}

fn main() {
    let id = String::from("foo");
    let mut map = IdOrdMap::<BorrowedItem<'_>>::new();
    map.insert_unique(BorrowedItem { id: &id, value: 0 }).unwrap();
    map.get_mut("foo").unwrap().value = 1;
    map.retain(|item| item.value > 0);
    map.entry("foo").and_modify(|mut item| item.value = 2);
}
