// A `Serialize` impl that exists only for `Key<'static>` must not be usable
// to serialize a map that lives shorter than `'static`: the key handed to
// `Serialize::serialize` borrows from the map.

use iddqd::{IdOrdItem, IdOrdMap, id_ord_map::IdOrdMapAsMap, id_upcast};
use serde::{Serialize, Serializer};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Key<'a>(&'a str);

impl Serialize for Key<'static> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.0)
    }
}

#[derive(Debug)]
struct Item {
    id: String,
}

impl Serialize for Item {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.id)
    }
}

impl IdOrdItem for Item {
    type Key<'a> = Key<'a>;

    fn key(&self) -> Self::Key<'_> {
        Key(&self.id)
    }

    id_upcast!();
}

fn serialize<S: Serializer>(s: S) -> Result<S::Ok, S::Error> {
    let mut map = IdOrdMap::new();
    map.insert_unique(Item { id: "foo".to_owned() }).unwrap();
    IdOrdMapAsMap::serialize(&map, s)
}

fn main() {}
