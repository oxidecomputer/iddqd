//! Schemars implementations for IdOrdMap.

use crate::{
    id_ord_map::{imp::IdOrdMap, trait_defs::IdOrdItem},
    support::schemars_utils::create_map_schema,
};
use alloc::string::String;
use schemars::{JsonSchema, gen::SchemaGenerator, schema::Schema};

impl<T> JsonSchema for IdOrdMap<T>
where
    T: JsonSchema + IdOrdItem,
{
    fn schema_name() -> String {
        alloc::format!("IdOrdMap_of_{}", T::schema_name())
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        create_map_schema::<T>("IdOrdMap", "iddqd::IdOrdMap", generator)
    }

    fn is_referenceable() -> bool {
        false
    }
}
