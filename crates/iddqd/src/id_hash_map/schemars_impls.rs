//! Schemars implementations for IdHashMap.

use crate::{
    id_hash_map::{imp::IdHashMap, trait_defs::IdHashItem},
    support::{alloc::Allocator, schemars_utils::create_map_schema},
};
use alloc::string::String;
use schemars::{JsonSchema, gen::SchemaGenerator, schema::Schema};

impl<T, S, A> JsonSchema for IdHashMap<T, S, A>
where
    T: JsonSchema + IdHashItem,
    A: Allocator,
{
    fn schema_name() -> String {
        alloc::format!("IdHashMap_of_{}", T::schema_name())
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        create_map_schema::<T>("IdHashMap", "iddqd::IdHashMap", generator)
    }

    fn is_referenceable() -> bool {
        false
    }
}
