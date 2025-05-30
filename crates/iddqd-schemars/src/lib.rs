//! JsonSchema implementations for iddqd map types using [schemars
//! 0.8](schemars).
//!
//! This crate provides JSON Schema generation support for the various map types
//! in the [iddqd](https://crates.io/crates/iddqd) crate.
//!
//! All map types serialize as arrays of their values (matching their serde
//! serialization format), so the JSON schemas generated reflect this structure.
//!
//! # Usage
//!
//! Use the marker types in this crate with the `#[schemars(with = "Type")]`
//! attribute.
//!
//! ## Examples
//!
//! ```
//! use iddqd::{IdHashItem, IdHashMap, id_upcast};
//! use iddqd_schemars::IdHashMapSchema;
//! use schemars::{JsonSchema, schema_for};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Serialize, Deserialize, JsonSchema)]
//! struct User {
//!     name: String,
//!     age: u32,
//! }
//!
//! impl IdHashItem for User {
//!     type Key<'a> = &'a str;
//!
//!     fn key(&self) -> Self::Key<'_> {
//!         &self.name
//!     }
//!
//!     id_upcast!();
//! }
//!
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! struct MyStruct {
//!     #[schemars(with = "IdHashMapSchema<User>")]
//!     users: IdHashMap<User>,
//! }
//!
//! // Generate schema for MyStruct
//! let schema = schema_for!(MyStruct);
//! ```
//!
//! ## `x-rust-type` and typify integration
//!
//! The schemars in this crate produce an extra `x-rust-type` extension field
//! that can be used to integrate with [typify] by specifying `iddqd` in the
//! `crates` table. See the [`typify-types` example].
//!
//! # Why is this its own crate?
//!
//! If this were a feature in the iddqd crate, it wouldn't be possible for
//! dependencies of schemars to use iddqd. Having these implementations live in
//! a separate crate works around this circular dependency limitation.
//!
//! [typify]: https://crates.io/crates/typify
//! [`typify-types` example]:
//!   https://github.com/oxidecomputer/iddqd/blob/main/crates/iddqd-schemars/examples/typify-types.rs

#![cfg_attr(doc_cfg, feature(doc_auto_cfg))]
#![warn(missing_docs)]

static IDDQD_CRATE_NAME: &str = "iddqd";
static IDDQD_CRATE_VERSION: &str = "0.3.0";

use schemars::{
    JsonSchema,
    gen::SchemaGenerator,
    schema::{Schema, SchemaObject},
};
use serde::Serialize;
use std::{boxed::Box, collections::BTreeMap, marker::PhantomData};

/// Marker type for [`JsonSchema`] generation of `IdHashMap<T>`.
///
/// Use this with `#[schemars(with = "IdHashMapSchema<T>")]`, where `T` is your
/// value type.
///
/// # Example
///
/// ```
/// use iddqd::{IdHashItem, IdHashMap, id_upcast};
/// use iddqd_schemars::IdHashMapSchema;
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// struct User {
///     name: String,
///     age: u32,
/// }
///
/// impl IdHashItem for User {
///     type Key<'a> = &'a str;
///     fn key(&self) -> Self::Key<'_> {
///         &self.name
///     }
///     id_upcast!();
/// }
///
/// #[derive(Serialize, Deserialize, JsonSchema)]
/// struct Container {
///     #[schemars(with = "IdHashMapSchema<User>")]
///     users: IdHashMap<User>,
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct IdHashMapSchema<T>(
    // Implementation note: here and below, we use fn() -> T to make this type
    // Send + Sync regardless of T. See
    // https://doc.rust-lang.org/nomicon/phantom-data.html#table-of-phantomdata-patterns.
    PhantomData<fn() -> T>,
);

impl<T> JsonSchema for IdHashMapSchema<T>
where
    T: JsonSchema,
{
    fn schema_name() -> String {
        format!("IdHashMap_of_{}", T::schema_name())
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Array.into()),
            array: Some(Box::new(schemars::schema::ArrayValidation {
                items: Some(schemars::schema::SingleOrVec::Single(Box::new(
                    generator.subschema_for::<T>(),
                ))),
                ..Default::default()
            })),
            metadata: Some(Box::new(schemars::schema::Metadata {
                title: Some("IdHashMap".to_string()),
                description: Some(
                    "A hash map where keys are borrowed from values, \
                     serialized as an array of values"
                        .to_string(),
                ),
                ..Default::default()
            })),
            extensions: make_extension_table::<T>(
                "iddqd::IdHashMap",
                generator,
            ),
            ..Default::default()
        })
    }
}

/// Marker type for [`JsonSchema`] generation of `IdOrdMap<T>`.
///
/// Use this with `#[schemars(with = "IdOrdMapSchema<T>")]` where `T` is your value type.
///
/// # Example
///
/// ```
/// # #[cfg(feature = "std")] {
/// use iddqd::{IdOrdItem, IdOrdMap, id_upcast};
/// use iddqd_schemars::IdOrdMapSchema;
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// struct User {
///     name: String,
///     age: u32,
/// }
///
/// impl IdOrdItem for User {
///     type Key<'a> = &'a str;
///     fn key(&self) -> Self::Key<'_> {
///         &self.name
///     }
///     fn upcast_key<'short, 'long: 'short>(
///         long: Self::Key<'long>,
///     ) -> Self::Key<'short> {
///         long
///     }
/// }
///
/// #[derive(Serialize, Deserialize, JsonSchema)]
/// struct Container {
///     #[schemars(with = "IdOrdMapSchema<User>")]
///     users: IdOrdMap<User>,
/// }
/// # }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct IdOrdMapSchema<T>(PhantomData<fn() -> T>);

impl<T> JsonSchema for IdOrdMapSchema<T>
where
    T: JsonSchema,
{
    fn schema_name() -> String {
        format!("IdOrdMap_of_{}", T::schema_name())
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Array.into()),
            array: Some(Box::new(schemars::schema::ArrayValidation {
                items: Some(schemars::schema::SingleOrVec::Single(Box::new(
                    generator.subschema_for::<T>(),
                ))),
                ..Default::default()
            })),
            metadata: Some(Box::new(schemars::schema::Metadata {
                title: Some("IdOrdMap".to_string()),
                description: Some(
                    "An ordered map where keys are borrowed from values, \
                     serialized as an array of values"
                        .to_string(),
                ),
                ..Default::default()
            })),
            extensions: make_extension_table::<T>("iddqd::IdOrdMap", generator),
            ..Default::default()
        })
    }
}

/// Marker type for [`JsonSchema`] generation of `BiHashMap<T>`.
///
/// Use this with `#[schemars(with = "BiHashMapSchema<T>")]`, where `T` is your
/// value type.
///
/// # Example
///
/// ```
/// use iddqd::{BiHashItem, BiHashMap, bi_upcast};
/// use iddqd_schemars::BiHashMapSchema;
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// struct User {
///     name: String,
///     email: String,
///     age: u32,
/// }
///
/// impl BiHashItem for User {
///     type K1<'a> = &'a str;
///     type K2<'a> = &'a str;
///     fn key1(&self) -> Self::K1<'_> {
///         &self.name
///     }
///     fn key2(&self) -> Self::K2<'_> {
///         &self.email
///     }
///     bi_upcast!();
/// }
///
/// #[derive(Serialize, Deserialize, JsonSchema)]
/// struct Container {
///     #[schemars(with = "BiHashMapSchema<User>")]
///     users: BiHashMap<User>,
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct BiHashMapSchema<T>(PhantomData<fn() -> T>);

impl<T> JsonSchema for BiHashMapSchema<T>
where
    T: JsonSchema,
{
    fn schema_name() -> String {
        format!("BiHashMap_of_{}", T::schema_name())
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Array.into()),
            array: Some(Box::new(schemars::schema::ArrayValidation {
                items: Some(schemars::schema::SingleOrVec::Single(Box::new(
                    generator.subschema_for::<T>(),
                ))),
                ..Default::default()
            })),
            metadata: Some(Box::new(schemars::schema::Metadata {
                title: Some("BiHashMap".to_string()),
                description: Some(
                    "A bijective hash map with two keys, \
                     serialized as an array of values"
                        .to_string(),
                ),
                ..Default::default()
            })),
            extensions: make_extension_table::<T>(
                "iddqd::BiHashMap",
                generator,
            ),
            ..Default::default()
        })
    }
}

/// Marker type for [`JsonSchema`] generation of `TriHashMap<T>`.
///
/// Use this with `#[schemars(with = "TriHashMapSchema<T>")]`, where `T` is your
/// value type.
///
/// # Example
///
/// ```
/// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
/// use iddqd_schemars::TriHashMapSchema;
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize, Deserialize, JsonSchema)]
/// struct User {
///     id: u32,
///     name: String,
///     email: String,
///     age: u32,
/// }
///
/// impl TriHashItem for User {
///     type K1<'a> = u32;
///     type K2<'a> = &'a str;
///     type K3<'a> = &'a str;
///     fn key1(&self) -> Self::K1<'_> {
///         self.id
///     }
///     fn key2(&self) -> Self::K2<'_> {
///         &self.name
///     }
///     fn key3(&self) -> Self::K3<'_> {
///         &self.email
///     }
///     tri_upcast!();
/// }
///
/// #[derive(Serialize, Deserialize, JsonSchema)]
/// struct Container {
///     #[schemars(with = "TriHashMapSchema<User>")]
///     users: TriHashMap<User>,
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct TriHashMapSchema<T>(PhantomData<fn() -> T>);

impl<T> JsonSchema for TriHashMapSchema<T>
where
    T: JsonSchema,
{
    fn schema_name() -> String {
        format!("TriHashMap_of_{}", T::schema_name())
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        Schema::Object(SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Array.into()),
            array: Some(Box::new(schemars::schema::ArrayValidation {
                items: Some(schemars::schema::SingleOrVec::Single(Box::new(
                    generator.subschema_for::<T>(),
                ))),
                ..Default::default()
            })),
            metadata: Some(Box::new(schemars::schema::Metadata {
                title: Some("TriHashMap".to_string()),
                description: Some(
                    "A trijective hash map with three keys, \
                     serialized as an array of values"
                        .to_string(),
                ),
                ..Default::default()
            })),
            extensions: make_extension_table::<T>(
                "iddqd::TriHashMap",
                generator,
            ),
            ..Default::default()
        })
    }
}

// https://github.com/oxidecomputer/typify#including-x-rust-type-in-your-library
#[derive(Serialize)]
struct XRustType {
    #[serde(rename = "crate")]
    crate_: &'static str,
    version: &'static str,
    path: &'static str,
    parameters: Vec<Schema>,
}

/// Helper function to create the `extension` table for a given path and
/// type parameter.
fn make_extension_table<T>(
    path: &'static str,
    generator: &mut SchemaGenerator,
) -> BTreeMap<String, serde_json::Value>
where
    T: JsonSchema,
{
    [(
        "x-rust-type".to_string(),
        serde_json::to_value(XRustType {
            crate_: IDDQD_CRATE_NAME,
            version: IDDQD_CRATE_VERSION,
            path,
            parameters: vec![generator.subschema_for::<T>()],
        })
        .expect("x-rust-type converted to serde_json::Value"),
    )]
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use expectorate::assert_contents;
    use iddqd::{
        BiHashItem, BiHashMap, IdHashItem, IdHashMap, IdOrdItem, IdOrdMap,
        TriHashItem, TriHashMap, bi_upcast, id_upcast, tri_upcast,
    };
    use schemars::{JsonSchema, schema_for};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    struct TestUser {
        id: u32,
        name: String,
        email: String,
        age: u32,
    }

    impl IdHashItem for TestUser {
        type Key<'a> = &'a str;

        fn key(&self) -> Self::Key<'_> {
            &self.name
        }

        id_upcast!();
    }

    impl BiHashItem for TestUser {
        type K1<'a> = &'a str;
        type K2<'a> = u32;

        fn key1(&self) -> Self::K1<'_> {
            &self.name
        }

        fn key2(&self) -> Self::K2<'_> {
            self.id
        }

        bi_upcast!();
    }

    impl TriHashItem for TestUser {
        type K1<'a> = &'a str;
        type K2<'a> = u32;
        type K3<'a> = &'a str;

        fn key1(&self) -> Self::K1<'_> {
            &self.name
        }

        fn key2(&self) -> Self::K2<'_> {
            self.id
        }

        fn key3(&self) -> Self::K3<'_> {
            &self.email
        }

        tri_upcast!();
    }

    impl IdOrdItem for TestUser {
        type Key<'a> = &'a str;

        fn key(&self) -> Self::Key<'_> {
            &self.name
        }

        id_upcast!();
    }

    #[test]
    fn schema_fixtures() {
        let schema = schema_for!(IdHashMapSchema<TestUser>);
        assert_contents(
            "tests/output/id_hash_map_schema.json",
            &serde_json::to_string_pretty(&schema).unwrap(),
        );

        let schema = schema_for!(IdOrdMapSchema<TestUser>);
        assert_contents(
            "tests/output/id_ord_map_schema.json",
            &serde_json::to_string_pretty(&schema).unwrap(),
        );

        let schema = schema_for!(BiHashMapSchema<TestUser>);
        assert_contents(
            "tests/output/bi_hash_map_schema.json",
            &serde_json::to_string_pretty(&schema).unwrap(),
        );

        let schema = schema_for!(TriHashMapSchema<TestUser>);
        assert_contents(
            "tests/output/tri_hash_map_schema.json",
            &serde_json::to_string_pretty(&schema).unwrap(),
        );
    }

    #[test]
    fn container_fixtures() {
        #[derive(JsonSchema)]
        #[expect(unused)]
        struct Container {
            #[schemars(with = "IdHashMapSchema<TestUser>")]
            users_hash: IdHashMap<TestUser>,

            #[schemars(with = "BiHashMapSchema<TestUser>")]
            users_bi: BiHashMap<TestUser>,

            #[schemars(with = "TriHashMapSchema<TestUser>")]
            users_tri: TriHashMap<TestUser>,

            #[schemars(with = "IdOrdMapSchema<TestUser>")]
            users_ord: IdOrdMap<TestUser>,
        }

        // Verify the container can generate a schema.
        let schema = schema_for!(Container);
        assert_contents(
            "tests/output/container_schema.json",
            &serde_json::to_string_pretty(&schema).unwrap(),
        );

        // A simple container with just IdHashMap<TestUser>. This fixture is
        // used by `typify-types.rs` to show end-to-end usage.
        #[derive(JsonSchema)]
        #[expect(unused)]
        struct SimpleContainer {
            #[schemars(with = "IdHashMapSchema<TestUser>")]
            users: IdHashMap<TestUser>,
        }

        let schema = schema_for!(SimpleContainer);
        assert_contents(
            "tests/output/simple_container_schema.json",
            &serde_json::to_string_pretty(&schema).unwrap(),
        );
    }
}
