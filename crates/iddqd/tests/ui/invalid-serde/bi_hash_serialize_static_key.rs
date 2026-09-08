// A `Serialize` impl that exists only for `Key<'static>` must not be usable
// to serialize a map that lives shorter than `'static`: the key handed to
// `Serialize::serialize` borrows from the map.

use iddqd::{BiHashItem, BiHashMap, bi_hash_map::BiHashMapAsMap, bi_upcast};
use serde::{Serialize, Serializer};

#[derive(Debug, PartialEq, Eq, Hash)]
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

impl BiHashItem for Item {
    type K1<'a> = Key<'a>;
    type K2<'a> = Key<'a>;

    fn key1(&self) -> Self::K1<'_> {
        Key(&self.id)
    }

    fn key2(&self) -> Self::K2<'_> {
        Key(&self.id)
    }

    bi_upcast!();
}

fn serialize<S: Serializer>(s: S) -> Result<S::Ok, S::Error> {
    let mut map = BiHashMap::new();
    map.insert_unique(Item { id: "foo".to_owned() }).unwrap();
    BiHashMapAsMap::serialize(&map, s)
}

fn main() {}
