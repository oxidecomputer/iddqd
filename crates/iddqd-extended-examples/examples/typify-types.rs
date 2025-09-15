//! Example showing how to use the `x-rust-type` extension with [typify].
//!
//! For this example, we use the `simple_container_schema.json` file generated
//! by one of our tests.

use iddqd::{IdHashItem, id_upcast};
use typify::import_types;

import_types!(
    // Import types from the schema file.
    schema = "../iddqd/tests/output/simple_container_schema.json",
    // Add iddqd to your dependency list, and specify that you have the "iddqd"
    // crate available.
    crates = {
        "iddqd" = "0.3.13",
    },
);

// You'll have to either implement the iddqd traits for the item types (in this
// case, TestUser), or use `replace` if the original type is available.
//
// If you're implementing the trait yourself, be sure to match the key type(s)
// with the original implementation! Information about which field(s) form the
// key is not part of the schema.
impl IdHashItem for TestUser {
    type Key<'a> = &'a str;

    fn key(&self) -> Self::Key<'_> {
        &self.name
    }

    id_upcast!();
}

fn main() {
    // Here's an example JSON value that represents an IdHashMap serialized as a
    // JSON array.
    let value = serde_json::json!({
        "users": [{
            "id": 20,
            "name": "Alice",
            "age": 30,
            "email": "alice@example.com"
        }],
    });

    // Deserialize the value into a IdHashMap<TestUser>.
    let container = serde_json::from_value::<SimpleContainer>(value).unwrap();

    // Get the user from the `IdHashMap`.
    let user = container.users.get("Alice").unwrap();
    println!("user: {user:?}");
}
