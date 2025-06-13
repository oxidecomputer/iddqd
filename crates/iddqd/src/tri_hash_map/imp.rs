use super::{IntoIter, Iter, IterMut, RefMut, tables::TriHashMapTables};
use crate::{
    DefaultHashBuilder, TriHashItem,
    errors::DuplicateItem,
    internal::ValidationError,
    support::{
        alloc::{AllocWrapper, Allocator, Global, global_alloc},
        borrow::DormantMutRef,
        fmt_utils::StrDisplayAsDebug,
        item_set::ItemSet,
    },
};
use alloc::{collections::BTreeSet, vec::Vec};
use core::{
    fmt,
    hash::{BuildHasher, Hash},
};
use equivalent::Equivalent;
use hashbrown::hash_table::{Entry, VacantEntry};

/// A 1:1:1 (trijective) map for three keys and a value.
///
/// The storage mechanism is a fast hash table of integer indexes to items, with
/// these indexes stored in three hashmaps. This allows for efficient lookups by
/// any of the three keys, while preventing duplicates.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "default-hasher")] {
/// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
///
/// #[derive(Debug, PartialEq, Eq)]
/// struct Person {
///     id: u32,
///     email: String,
///     phone: String,
///     name: String,
/// }
///
/// // Implement TriHashItem to define the three key types.
/// impl TriHashItem for Person {
///     type K1<'a> = u32;
///     type K2<'a> = &'a str;
///     type K3<'a> = &'a str;
///
///     fn key1(&self) -> Self::K1<'_> {
///         self.id
///     }
///
///     fn key2(&self) -> Self::K2<'_> {
///         &self.email
///     }
///
///     fn key3(&self) -> Self::K3<'_> {
///         &self.phone
///     }
///
///     tri_upcast!();
/// }
///
/// // Create a TriHashMap and insert items.
/// let mut people = TriHashMap::new();
/// people
///     .insert_unique(Person {
///         id: 1,
///         email: "alice@example.com".to_string(),
///         phone: "555-1234".to_string(),
///         name: "Alice".to_string(),
///     })
///     .unwrap();
///
/// // Lookup by any of the three keys.
/// let person = people.get1(&1).unwrap();
/// assert_eq!(person.name, "Alice");
///
/// let person = people.get2("alice@example.com").unwrap();
/// assert_eq!(person.id, 1);
///
/// let person = people.get3("555-1234").unwrap();
/// assert_eq!(person.email, "alice@example.com");
/// # }
/// ```
#[derive(Clone)]
pub struct TriHashMap<
    T: TriHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    pub(super) items: ItemSet<T, A>,
    // Invariant: the values (usize) in these tables are valid indexes into
    // `items`, and are a 1:1 mapping.
    tables: TriHashMapTables<S, A>,
}

impl<T: TriHashItem, S: Default, A: Allocator + Default> Default
    for TriHashMap<T, S, A>
{
    fn default() -> Self {
        Self {
            items: ItemSet::with_capacity_in(0, A::default()),
            tables: TriHashMapTables::default(),
        }
    }
}

#[cfg(feature = "default-hasher")]
impl<T: TriHashItem> TriHashMap<T> {
    /// Creates a new, empty `TriHashMap`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let map: TriHashMap<Person> = TriHashMap::new();
    /// assert!(map.is_empty());
    /// assert_eq!(map.len(), 0);
    /// # }
    /// ```
    #[inline]
    pub fn new() -> Self {
        Self { items: ItemSet::default(), tables: TriHashMapTables::default() }
    }

    /// Creates a new `TriHashMap` with the given capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let map: TriHashMap<Person> = TriHashMap::with_capacity(10);
    /// assert!(map.capacity() >= 10);
    /// assert!(map.is_empty());
    /// # }
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: ItemSet::with_capacity_in(capacity, global_alloc()),
            tables: TriHashMapTables::with_capacity_and_hasher_in(
                capacity,
                DefaultHashBuilder::default(),
                global_alloc(),
            ),
        }
    }
}

impl<T: TriHashItem, S: Clone + BuildHasher> TriHashMap<T, S> {
    /// Creates a new, empty `TriHashMap` with the given hasher.
    ///
    /// # Examples
    ///
    /// ```
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    /// use std::collections::hash_map::RandomState;
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let map: TriHashMap<Person, RandomState> =
    ///     TriHashMap::with_hasher(RandomState::new());
    /// assert!(map.is_empty());
    /// ```
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            items: ItemSet::default(),
            tables: TriHashMapTables::with_capacity_and_hasher_in(
                0,
                hasher.clone(),
                global_alloc(),
            ),
        }
    }

    /// Creates a new `TriHashMap` with the given capacity and hasher.
    ///
    /// # Examples
    ///
    /// ```
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    /// use std::collections::hash_map::RandomState;
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let map: TriHashMap<Person, RandomState> =
    ///     TriHashMap::with_capacity_and_hasher(10, RandomState::new());
    /// assert!(map.capacity() >= 10);
    /// assert!(map.is_empty());
    /// ```
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self {
            items: ItemSet::with_capacity_in(capacity, global_alloc()),
            tables: TriHashMapTables::with_capacity_and_hasher_in(
                capacity,
                hasher,
                global_alloc(),
            ),
        }
    }
}

#[cfg(feature = "default-hasher")]
impl<T: TriHashItem, A: Clone + Allocator>
    TriHashMap<T, DefaultHashBuilder, A>
{
    /// Creates a new empty `TriHashMap` using the given allocator.
    ///
    /// Requires the `allocator-api2` feature to be enabled.
    ///
    /// # Examples
    ///
    /// Using the [`bumpalo`](https://docs.rs/bumpalo) allocator:
    ///
    /// ```
    /// # #[cfg(all(feature = "default-hasher", feature = "allocator-api2"))] {
    /// use iddqd::{TriHashMap, TriHashItem, tri_upcast};
    /// # use iddqd_test_utils::bumpalo;
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// // Define a new allocator.
    /// let bump = bumpalo::Bump::new();
    /// // Create a new TriHashMap using the allocator.
    /// let map: TriHashMap<Person, _, &bumpalo::Bump> = TriHashMap::new_in(&bump);
    /// assert!(map.is_empty());
    /// # }
    /// ```
    pub fn new_in(alloc: A) -> Self {
        Self {
            items: ItemSet::with_capacity_in(0, alloc.clone()),
            tables: TriHashMapTables::with_capacity_and_hasher_in(
                0,
                DefaultHashBuilder::default(),
                alloc,
            ),
        }
    }

    /// Creates an empty `TriHashMap` with the specified capacity using the given
    /// allocator.
    ///
    /// Requires the `allocator-api2` feature to be enabled.
    ///
    /// # Examples
    ///
    /// Using the [`bumpalo`](https://docs.rs/bumpalo) allocator:
    ///
    /// ```
    /// # #[cfg(all(feature = "default-hasher", feature = "allocator-api2"))] {
    /// use iddqd::{TriHashMap, TriHashItem, tri_upcast};
    /// # use iddqd_test_utils::bumpalo;
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// // Define a new allocator.
    /// let bump = bumpalo::Bump::new();
    /// // Create a new TriHashMap with capacity using the allocator.
    /// let map: TriHashMap<Person, _, &bumpalo::Bump> = TriHashMap::with_capacity_in(10, &bump);
    /// assert!(map.capacity() >= 10);
    /// assert!(map.is_empty());
    /// # }
    /// ```
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self {
            items: ItemSet::with_capacity_in(capacity, alloc.clone()),
            tables: TriHashMapTables::with_capacity_and_hasher_in(
                capacity,
                DefaultHashBuilder::default(),
                alloc,
            ),
        }
    }
}

impl<T: TriHashItem, S: Clone + BuildHasher, A: Clone + Allocator>
    TriHashMap<T, S, A>
{
    /// Creates a new, empty `TriHashMap` with the given hasher and allocator.
    ///
    /// Requires the `allocator-api2` feature to be enabled.
    ///
    /// # Examples
    ///
    /// Using the [`bumpalo`](https://docs.rs/bumpalo) allocator:
    ///
    /// ```
    /// # #[cfg(feature = "allocator-api2")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    /// use std::collections::hash_map::RandomState;
    /// # use iddqd_test_utils::bumpalo;
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// // Define a new allocator.
    /// let bump = bumpalo::Bump::new();
    /// let hasher = RandomState::new();
    /// // Create a new TriHashMap with hasher using the allocator.
    /// let map: TriHashMap<Person, _, &bumpalo::Bump> =
    ///     TriHashMap::with_hasher_in(hasher, &bump);
    /// assert!(map.is_empty());
    /// # }
    /// ```
    pub fn with_hasher_in(hasher: S, alloc: A) -> Self {
        Self {
            items: ItemSet::with_capacity_in(0, alloc.clone()),
            tables: TriHashMapTables::with_capacity_and_hasher_in(
                0,
                hasher.clone(),
                alloc,
            ),
        }
    }

    /// Creates a new `TriHashMap` with the given capacity, hasher, and
    /// allocator.
    ///
    /// Requires the `allocator-api2` feature to be enabled.
    ///
    /// # Examples
    ///
    /// Using the [`bumpalo`](https://docs.rs/bumpalo) allocator:
    ///
    /// ```
    /// # #[cfg(feature = "allocator-api2")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    /// use std::collections::hash_map::RandomState;
    /// # use iddqd_test_utils::bumpalo;
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// // Define a new allocator.
    /// let bump = bumpalo::Bump::new();
    /// let hasher = RandomState::new();
    /// // Create a new TriHashMap with capacity and hasher using the allocator.
    /// let map: TriHashMap<Person, _, &bumpalo::Bump> =
    ///     TriHashMap::with_capacity_and_hasher_in(10, hasher, &bump);
    /// assert!(map.capacity() >= 10);
    /// assert!(map.is_empty());
    /// # }
    /// ```
    pub fn with_capacity_and_hasher_in(
        capacity: usize,
        hasher: S,
        alloc: A,
    ) -> Self {
        Self {
            items: ItemSet::with_capacity_in(capacity, alloc.clone()),
            tables: TriHashMapTables::with_capacity_and_hasher_in(
                capacity, hasher, alloc,
            ),
        }
    }
}

impl<T: TriHashItem, S: Clone + BuildHasher, A: Allocator> TriHashMap<T, S, A> {
    /// Returns the hasher.
    #[cfg(feature = "daft")]
    #[inline]
    pub(crate) fn hasher(&self) -> &S {
        self.tables.hasher()
    }

    /// Returns the allocator.
    ///
    /// Requires the `allocator-api2` feature to be enabled.
    ///
    /// # Examples
    ///
    /// Using the [`bumpalo`](https://docs.rs/bumpalo) allocator:
    ///
    /// ```
    /// # #[cfg(all(feature = "default-hasher", feature = "allocator-api2"))] {
    /// use iddqd::{TriHashMap, TriHashItem, tri_upcast};
    /// # use iddqd_test_utils::bumpalo;
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// // Define a new allocator.
    /// let bump = bumpalo::Bump::new();
    /// // Create a new TriHashMap using the allocator.
    /// let map: TriHashMap<Person, _, &bumpalo::Bump> = TriHashMap::new_in(&bump);
    /// // Access the allocator.
    /// let allocator = map.allocator();
    /// # }
    /// ```
    #[inline]
    pub fn allocator(&self) -> &A {
        self.items.allocator()
    }

    /// Returns the currently allocated capacity of the map.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let map: TriHashMap<Person> = TriHashMap::with_capacity(10);
    /// assert!(map.capacity() >= 10);
    /// # }
    /// ```
    pub fn capacity(&self) -> usize {
        // items and tables.capacity might theoretically diverge: use
        // items.capacity.
        self.items.capacity()
    }

    /// Returns true if the map is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// assert!(map.is_empty());
    ///
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    /// assert!(!map.is_empty());
    /// # }
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns the number of items in the map.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// assert_eq!(map.len(), 0);
    ///
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    /// map.insert_unique(Person {
    ///     id: 2,
    ///     email: "bob@example.com".to_string(),
    ///     phone: "555-5678".to_string(),
    ///     name: "Bob".to_string(),
    /// })
    /// .unwrap();
    /// assert_eq!(map.len(), 2);
    /// # }
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Iterates over the items in the map.
    ///
    /// Similar to [`HashMap`], the iteration order is arbitrary and not
    /// guaranteed to be stable.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    /// map.insert_unique(Person {
    ///     id: 2,
    ///     email: "bob@example.com".to_string(),
    ///     phone: "555-5678".to_string(),
    ///     name: "Bob".to_string(),
    /// })
    /// .unwrap();
    ///
    /// let mut count = 0;
    /// for person in map.iter() {
    ///     assert!(person.id == 1 || person.id == 2);
    ///     count += 1;
    /// }
    /// assert_eq!(count, 2);
    /// # }
    /// ```
    ///
    /// [`HashMap`]: std::collections::HashMap
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter::new(&self.items)
    }

    /// Iterates over the items in the map, allowing for mutation.
    ///
    /// Similar to [`HashMap`], the iteration order is arbitrary and not
    /// guaranteed to be stable.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// for mut person in map.iter_mut() {
    ///     person.name.push_str(" Updated");
    /// }
    ///
    /// let person = map.get1(&1).unwrap();
    /// assert_eq!(person.name, "Alice Updated");
    /// # }
    /// ```
    ///
    /// [`HashMap`]: std::collections::HashMap
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, T, S, A> {
        IterMut::new(&self.tables, &mut self.items)
    }

    /// Checks general invariants of the map.
    ///
    /// The code below always upholds these invariants, but it's useful to have
    /// an explicit check for tests.
    #[doc(hidden)]
    pub fn validate(
        &self,
        compactness: crate::internal::ValidateCompact,
    ) -> Result<(), ValidationError>
    where
        T: fmt::Debug,
    {
        self.items.validate(compactness)?;
        self.tables.validate(self.len(), compactness)?;

        // Check that the indexes are all correct.
        for (&ix, item) in self.items.iter() {
            let key1 = item.key1();
            let key2 = item.key2();
            let key3 = item.key3();

            let Some(ix1) = self.find1_index(&key1) else {
                return Err(ValidationError::general(format!(
                    "item at index {} has no key1 index",
                    ix
                )));
            };
            let Some(ix2) = self.find2_index(&key2) else {
                return Err(ValidationError::general(format!(
                    "item at index {} has no key2 index",
                    ix
                )));
            };
            let Some(ix3) = self.find3_index(&key3) else {
                return Err(ValidationError::general(format!(
                    "item at index {} has no key3 index",
                    ix
                )));
            };

            if ix1 != ix || ix2 != ix || ix3 != ix {
                return Err(ValidationError::general(format!(
                    "item at index {} has inconsistent indexes: {}/{}/{}",
                    ix, ix1, ix2, ix3
                )));
            }
        }

        Ok(())
    }

    /// Inserts a value into the map, removing any conflicting items and
    /// returning a list of those items.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    ///
    /// // First insertion - no conflicts
    /// let overwritten = map.insert_overwrite(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// });
    /// assert!(overwritten.is_empty());
    ///
    /// // Overwrite with same id - returns the old item
    /// let overwritten = map.insert_overwrite(Person {
    ///     id: 1,
    ///     email: "alice.new@example.com".to_string(),
    ///     phone: "555-9999".to_string(),
    ///     name: "Alice New".to_string(),
    /// });
    /// assert_eq!(overwritten.len(), 1);
    /// assert_eq!(overwritten[0].name, "Alice");
    /// # }
    /// ```
    #[doc(alias = "insert")]
    pub fn insert_overwrite(&mut self, value: T) -> Vec<T> {
        // Trying to write this function for maximal efficiency can get very
        // tricky, requiring delicate handling of indexes. We follow a very
        // simple approach instead:
        //
        // 1. Remove items corresponding to keys that are already in the map.
        // 2. Add the item to the map.

        let mut duplicates = Vec::new();
        duplicates.extend(self.remove1(&value.key1()));
        duplicates.extend(self.remove2(&value.key2()));
        duplicates.extend(self.remove3(&value.key3()));

        if self.insert_unique(value).is_err() {
            // We should never get here, because we just removed all the
            // duplicates.
            panic!("insert_unique failed after removing duplicates");
        }

        duplicates
    }

    /// Inserts a value into the set, returning an error if any duplicates were
    /// added.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    ///
    /// // Successful insertion
    /// assert!(
    ///     map.insert_unique(Person {
    ///         id: 1,
    ///         email: "alice@example.com".to_string(),
    ///         phone: "555-1234".to_string(),
    ///         name: "Alice".to_string(),
    ///     })
    ///     .is_ok()
    /// );
    /// assert!(
    ///     map.insert_unique(Person {
    ///         id: 2,
    ///         email: "bob@example.com".to_string(),
    ///         phone: "555-5678".to_string(),
    ///         name: "Bob".to_string(),
    ///     })
    ///     .is_ok()
    /// );
    ///
    /// // Duplicate key1
    /// assert!(
    ///     map.insert_unique(Person {
    ///         id: 1,
    ///         email: "charlie@example.com".to_string(),
    ///         phone: "555-9999".to_string(),
    ///         name: "Charlie".to_string(),
    ///     })
    ///     .is_err()
    /// );
    ///
    /// // Duplicate key2
    /// assert!(
    ///     map.insert_unique(Person {
    ///         id: 3,
    ///         email: "alice@example.com".to_string(),
    ///         phone: "555-7777".to_string(),
    ///         name: "Alice2".to_string(),
    ///     })
    ///     .is_err()
    /// );
    ///
    /// // Duplicate key3
    /// assert!(
    ///     map.insert_unique(Person {
    ///         id: 4,
    ///         email: "dave@example.com".to_string(),
    ///         phone: "555-1234".to_string(),
    ///         name: "Dave".to_string(),
    ///     })
    ///     .is_err()
    /// );
    /// # }
    /// ```
    pub fn insert_unique(
        &mut self,
        value: T,
    ) -> Result<(), DuplicateItem<T, &T>> {
        let mut duplicates = BTreeSet::new();

        // Check for duplicates *before* inserting the new item, because we
        // don't want to partially insert the new item and then have to roll
        // back.
        let (e1, e2, e3) = {
            let k1 = value.key1();
            let k2 = value.key2();
            let k3 = value.key3();

            let e1 = detect_dup_or_insert(
                self.tables
                    .k1_to_item
                    .entry(k1, |index| self.items[index].key1()),
                &mut duplicates,
            );
            let e2 = detect_dup_or_insert(
                self.tables
                    .k2_to_item
                    .entry(k2, |index| self.items[index].key2()),
                &mut duplicates,
            );
            let e3 = detect_dup_or_insert(
                self.tables
                    .k3_to_item
                    .entry(k3, |index| self.items[index].key3()),
                &mut duplicates,
            );
            (e1, e2, e3)
        };

        if !duplicates.is_empty() {
            return Err(DuplicateItem::__internal_new(
                value,
                duplicates.iter().map(|ix| &self.items[*ix]).collect(),
            ));
        }

        let next_index = self.items.insert_at_next_index(value);
        // e1, e2 and e3 are all Some because if they were None, duplicates
        // would be non-empty, and we'd have bailed out earlier.
        e1.unwrap().insert(next_index);
        e2.unwrap().insert(next_index);
        e3.unwrap().insert(next_index);

        Ok(())
    }

    /// Returns true if the map contains a single item that matches all three
    /// keys.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// }).unwrap();
    /// map.insert_unique(Person {
    ///     id: 2,
    ///     email: "bob@example.com".to_string(),
    ///     phone: "555-5678".to_string(),
    ///     name: "Bob".to_string(),
    /// }).unwrap();
    ///
    /// assert!(map.contains_key_unique(&1, &"alice@example.com", &"555-1234"));
    /// assert!(map.contains_key_unique(&2, &"bob@example.com", &"555-5678"));
    /// assert!(!map.contains_key_unique(&1, &"bob@example.com", &"555-1234")); // key1 exists but key2 doesn't match
    /// assert!(!map.contains_key_unique(&3, &"charlie@example.com", &"555-9999")); // none of the keys exist
    /// # }
    /// ```
    pub fn contains_key_unique<'a, Q1, Q2, Q3>(
        &'a self,
        key1: &Q1,
        key2: &Q2,
        key3: &Q3,
    ) -> bool
    where
        Q1: Hash + Equivalent<T::K1<'a>> + ?Sized,
        Q2: Hash + Equivalent<T::K2<'a>> + ?Sized,
        Q3: Hash + Equivalent<T::K3<'a>> + ?Sized,
    {
        self.get_unique(key1, key2, key3).is_some()
    }

    /// Gets a reference to the unique item associated with the given `key1`,
    /// `key2`, and `key3`, if it exists.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// // All three keys must match
    /// assert_eq!(
    ///     map.get_unique(&1, &"alice@example.com", &"555-1234").unwrap().name,
    ///     "Alice"
    /// );
    ///
    /// // If any key doesn't match, returns None
    /// assert!(map.get_unique(&1, &"wrong@example.com", &"555-1234").is_none());
    /// assert!(map.get_unique(&2, &"alice@example.com", &"555-1234").is_none());
    /// # }
    /// ```
    pub fn get_unique<'a, Q1, Q2, Q3>(
        &'a self,
        key1: &Q1,
        key2: &Q2,
        key3: &Q3,
    ) -> Option<&'a T>
    where
        Q1: Hash + Equivalent<T::K1<'a>> + ?Sized,
        Q2: Hash + Equivalent<T::K2<'a>> + ?Sized,
        Q3: Hash + Equivalent<T::K3<'a>> + ?Sized,
    {
        let index = self.find1_index(key1)?;
        let item = &self.items[index];
        if key2.equivalent(&item.key2()) && key3.equivalent(&item.key3()) {
            Some(item)
        } else {
            None
        }
    }

    /// Gets a mutable reference to the unique item associated with the given
    /// `key1`, `key2`, and `key3`, if it exists.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// // Modify the item through the mutable reference
    /// if let Some(mut person) =
    ///     map.get_mut_unique(&1, &"alice@example.com", &"555-1234")
    /// {
    ///     person.name = "Alice Updated".to_string();
    /// }
    ///
    /// // Verify the change
    /// assert_eq!(map.get1(&1).unwrap().name, "Alice Updated");
    /// # }
    /// ```
    pub fn get_mut_unique<'a, Q1, Q2, Q3>(
        &'a mut self,
        key1: &Q1,
        key2: &Q2,
        key3: &Q3,
    ) -> Option<RefMut<'a, T, S>>
    where
        Q1: Hash + Equivalent<T::K1<'a>> + ?Sized,
        Q2: Hash + Equivalent<T::K2<'a>> + ?Sized,
        Q3: Hash + Equivalent<T::K3<'a>> + ?Sized,
    {
        let (dormant_map, index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let index = map.find1_index(key1)?;
            let item = &map.items[index];
            if !key2.equivalent(&item.key2()) || !key3.equivalent(&item.key3())
            {
                return None;
            }
            (dormant_map, index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };
        let item = &mut awakened_map.items[index];
        let hashes = awakened_map.tables.make_hashes(&item);
        Some(RefMut::new(hashes, item))
    }

    /// Removes the item uniquely identified by `key1`, `key2`, and `key3`, if
    /// it exists.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// // Remove the item using all three keys
    /// let removed = map.remove_unique(&1, &"alice@example.com", &"555-1234");
    /// assert!(removed.is_some());
    /// assert_eq!(removed.unwrap().name, "Alice");
    ///
    /// // Map is now empty
    /// assert!(map.is_empty());
    ///
    /// // Trying to remove again returns None
    /// assert!(map.remove_unique(&1, &"alice@example.com", &"555-1234").is_none());
    /// # }
    /// ```
    pub fn remove_unique<'a, Q1, Q2, Q3>(
        &'a mut self,
        key1: &Q1,
        key2: &Q2,
        key3: &Q3,
    ) -> Option<T>
    where
        Q1: Hash + Equivalent<T::K1<'a>> + ?Sized,
        Q2: Hash + Equivalent<T::K2<'a>> + ?Sized,
        Q3: Hash + Equivalent<T::K3<'a>> + ?Sized,
    {
        let (dormant_map, remove_index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let remove_index = map.find1_index(key1)?;
            let item = &map.items[remove_index];
            if !key2.equivalent(&item.key2()) && !key3.equivalent(&item.key3())
            {
                return None;
            }
            (dormant_map, remove_index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };

        awakened_map.remove_by_index(remove_index)
    }

    /// Returns true if the map contains the given `key1`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// assert!(map.contains_key1(&1));
    /// assert!(!map.contains_key1(&2));
    /// # }
    /// ```
    pub fn contains_key1<'a, Q>(&'a self, key1: &Q) -> bool
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        self.find1_index(key1).is_some()
    }

    /// Gets a reference to the value associated with the given `key1`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// assert_eq!(map.get1(&1).unwrap().name, "Alice");
    /// assert!(map.get1(&2).is_none());
    /// # }
    /// ```
    pub fn get1<'a, Q>(&'a self, key1: &Q) -> Option<&'a T>
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        self.find1(key1)
    }

    /// Gets a mutable reference to the value associated with the given `key1`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// if let Some(mut person) = map.get1_mut(&1) {
    ///     person.name = "Alice Updated".to_string();
    /// }
    ///
    /// assert_eq!(map.get1(&1).unwrap().name, "Alice Updated");
    /// # }
    /// ```
    pub fn get1_mut<'a, Q>(&'a mut self, key1: &Q) -> Option<RefMut<'a, T, S>>
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        let (dormant_map, index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let index = map.find1_index(key1)?;
            (dormant_map, index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };
        let item = &mut awakened_map.items[index];
        let hashes = awakened_map.tables.make_hashes(&item);
        Some(RefMut::new(hashes, item))
    }

    /// Removes an item from the map by its `key1`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// let removed = map.remove1(&1);
    /// assert!(removed.is_some());
    /// assert_eq!(removed.unwrap().name, "Alice");
    /// assert!(map.is_empty());
    /// # }
    /// ```
    pub fn remove1<'a, Q>(&'a mut self, key1: &Q) -> Option<T>
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        let (dormant_map, remove_index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let remove_index = map.find1_index(key1)?;
            (dormant_map, remove_index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };

        awakened_map.remove_by_index(remove_index)
    }

    /// Returns true if the map contains the given `key2`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// assert!(map.contains_key2("alice@example.com"));
    /// assert!(!map.contains_key2("bob@example.com"));
    /// # }
    /// ```
    pub fn contains_key2<'a, Q>(&'a self, key2: &Q) -> bool
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        self.find2_index(key2).is_some()
    }

    /// Gets a reference to the value associated with the given `key2`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// assert_eq!(map.get2("alice@example.com").unwrap().name, "Alice");
    /// assert!(map.get2("bob@example.com").is_none());
    /// # }
    /// ```
    pub fn get2<'a, Q>(&'a self, key2: &Q) -> Option<&'a T>
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        self.find2(key2)
    }

    /// Gets a mutable reference to the value associated with the given `key2`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// if let Some(mut person) = map.get2_mut("alice@example.com") {
    ///     person.name = "Alice Updated".to_string();
    /// }
    ///
    /// assert_eq!(map.get2("alice@example.com").unwrap().name, "Alice Updated");
    /// # }
    /// ```
    pub fn get2_mut<'a, Q>(&'a mut self, key2: &Q) -> Option<RefMut<'a, T, S>>
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        let (dormant_map, index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let index = map.find2_index(key2)?;
            (dormant_map, index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };
        let item = &mut awakened_map.items[index];
        let hashes = awakened_map.tables.make_hashes(&item);
        Some(RefMut::new(hashes, item))
    }

    /// Removes an item from the map by its `key2`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// let removed = map.remove2("alice@example.com");
    /// assert!(removed.is_some());
    /// assert_eq!(removed.unwrap().name, "Alice");
    /// assert!(map.is_empty());
    /// # }
    /// ```
    pub fn remove2<'a, Q>(&'a mut self, key2: &Q) -> Option<T>
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        let (dormant_map, remove_index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let remove_index = map.find2_index(key2)?;
            (dormant_map, remove_index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };

        awakened_map.remove_by_index(remove_index)
    }

    /// Returns true if the map contains the given `key3`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// assert!(map.contains_key3("555-1234"));
    /// assert!(!map.contains_key3("555-5678"));
    /// # }
    /// ```
    pub fn contains_key3<'a, Q>(&'a self, key3: &Q) -> bool
    where
        Q: Hash + Equivalent<T::K3<'a>> + ?Sized,
    {
        self.find3_index(key3).is_some()
    }

    /// Gets a reference to the value associated with the given `key3`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// assert_eq!(map.get3("555-1234").unwrap().name, "Alice");
    /// assert!(map.get3("555-5678").is_none());
    /// # }
    /// ```
    pub fn get3<'a, Q>(&'a self, key3: &Q) -> Option<&'a T>
    where
        Q: Hash + Equivalent<T::K3<'a>> + ?Sized,
    {
        self.find3(key3)
    }

    /// Gets a mutable reference to the value associated with the given `key3`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// if let Some(mut person) = map.get3_mut("555-1234") {
    ///     person.name = "Alice Updated".to_string();
    /// }
    ///
    /// assert_eq!(map.get3("555-1234").unwrap().name, "Alice Updated");
    /// # }
    /// ```
    pub fn get3_mut<'a, Q>(&'a mut self, key3: &Q) -> Option<RefMut<'a, T, S>>
    where
        Q: Hash + Equivalent<T::K3<'a>> + ?Sized,
    {
        let (dormant_map, index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let index = map.find3_index(key3)?;
            (dormant_map, index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };
        let item = &mut awakened_map.items[index];
        let hashes = awakened_map.tables.make_hashes(&item);
        Some(RefMut::new(hashes, item))
    }

    /// Removes an item from the map by its `key3`.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[cfg(feature = "default-hasher")] {
    /// use iddqd::{TriHashItem, TriHashMap, tri_upcast};
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// struct Person {
    ///     id: u32,
    ///     email: String,
    ///     phone: String,
    ///     name: String,
    /// }
    ///
    /// impl TriHashItem for Person {
    ///     type K1<'a> = u32;
    ///     type K2<'a> = &'a str;
    ///     type K3<'a> = &'a str;
    ///
    ///     fn key1(&self) -> Self::K1<'_> {
    ///         self.id
    ///     }
    ///     fn key2(&self) -> Self::K2<'_> {
    ///         &self.email
    ///     }
    ///     fn key3(&self) -> Self::K3<'_> {
    ///         &self.phone
    ///     }
    ///     tri_upcast!();
    /// }
    ///
    /// let mut map = TriHashMap::new();
    /// map.insert_unique(Person {
    ///     id: 1,
    ///     email: "alice@example.com".to_string(),
    ///     phone: "555-1234".to_string(),
    ///     name: "Alice".to_string(),
    /// })
    /// .unwrap();
    ///
    /// let removed = map.remove3("555-1234");
    /// assert!(removed.is_some());
    /// assert_eq!(removed.unwrap().name, "Alice");
    /// assert!(map.is_empty());
    /// # }
    /// ```
    pub fn remove3<'a, Q>(&'a mut self, key3: &Q) -> Option<T>
    where
        Q: Hash + Equivalent<T::K3<'a>> + ?Sized,
    {
        let (dormant_map, remove_index) = {
            let (map, dormant_map) = DormantMutRef::new(self);
            let remove_index = map.find3_index(key3)?;
            (dormant_map, remove_index)
        };

        // SAFETY: `map` is not used after this point.
        let awakened_map = unsafe { dormant_map.awaken() };

        awakened_map.remove_by_index(remove_index)
    }

    fn find1<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        self.find1_index(k).map(|ix| &self.items[ix])
    }

    fn find1_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        Q: Hash + Equivalent<T::K1<'a>> + ?Sized,
    {
        self.tables.k1_to_item.find_index(k, |index| self.items[index].key1())
    }

    fn find2<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        self.find2_index(k).map(|ix| &self.items[ix])
    }

    fn find2_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        Q: Hash + Equivalent<T::K2<'a>> + ?Sized,
    {
        self.tables.k2_to_item.find_index(k, |index| self.items[index].key2())
    }

    fn find3<'a, Q>(&'a self, k: &Q) -> Option<&'a T>
    where
        Q: Hash + Equivalent<T::K3<'a>> + ?Sized,
    {
        self.find3_index(k).map(|ix| &self.items[ix])
    }

    fn find3_index<'a, Q>(&'a self, k: &Q) -> Option<usize>
    where
        Q: Hash + Equivalent<T::K3<'a>> + ?Sized,
    {
        self.tables.k3_to_item.find_index(k, |index| self.items[index].key3())
    }

    pub(super) fn remove_by_index(&mut self, remove_index: usize) -> Option<T> {
        let value = self.items.remove(remove_index)?;

        // Remove the value from the tables.
        let Ok(item1) =
            self.tables.k1_to_item.find_entry(&value.key1(), |index| {
                if index == remove_index {
                    value.key1()
                } else {
                    self.items[index].key1()
                }
            })
        else {
            // The item was not found.
            panic!("remove_index {remove_index} not found in k1_to_item");
        };
        let Ok(item2) =
            self.tables.k2_to_item.find_entry(&value.key2(), |index| {
                if index == remove_index {
                    value.key2()
                } else {
                    self.items[index].key2()
                }
            })
        else {
            // The item was not found.
            panic!("remove_index {remove_index} not found in k2_to_item")
        };
        let Ok(item3) =
            self.tables.k3_to_item.find_entry(&value.key3(), |index| {
                if index == remove_index {
                    value.key3()
                } else {
                    self.items[index].key3()
                }
            })
        else {
            // The item was not found.
            panic!("remove_index {remove_index} not found in k3_to_item")
        };

        item1.remove();
        item2.remove();
        item3.remove();

        Some(value)
    }
}

impl<'a, T, S, A: Allocator> fmt::Debug for TriHashMap<T, S, A>
where
    T: TriHashItem + fmt::Debug,
    T::K1<'a>: fmt::Debug,
    T::K2<'a>: fmt::Debug,
    T::K3<'a>: fmt::Debug,
    T: 'a,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();
        for item in self.items.values() {
            let key: KeyMap<'_, T> = KeyMap {
                key1: item.key1(),
                key2: item.key2(),
                key3: item.key3(),
            };

            // SAFETY:
            //
            // * Lifetime extension: for a type T and two lifetime params 'a and
            //   'b, T<'a> and T<'b> aren't guaranteed to have the same layout,
            //   but (a) that is true today and (b) it would be shocking and
            //   break half the Rust ecosystem if that were to change in the
            //   future.
            // * We only use key within the scope of this block before immediately
            //   dropping it. In particular, map.entry calls key.fmt() without
            //   holding a reference to it.
            let key: KeyMap<'a, T> = unsafe {
                core::mem::transmute::<KeyMap<'_, T>, KeyMap<'a, T>>(key)
            };

            map.entry(&key, item);
        }
        map.finish()
    }
}

struct KeyMap<'a, T: TriHashItem + 'a> {
    key1: T::K1<'a>,
    key2: T::K2<'a>,
    key3: T::K3<'a>,
}

impl<'a, T: TriHashItem> fmt::Debug for KeyMap<'a, T>
where
    T::K1<'a>: fmt::Debug,
    T::K2<'a>: fmt::Debug,
    T::K3<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We don't want to show key1 and key2 as a tuple since it's
        // misleading (suggests maps of tuples). The best we can do
        // instead is to show "{k1: abc, k2: xyz, k3: def}"
        f.debug_map()
            .entry(&StrDisplayAsDebug("k1"), &self.key1)
            .entry(&StrDisplayAsDebug("k2"), &self.key2)
            .entry(&StrDisplayAsDebug("k3"), &self.key3)
            .finish()
    }
}

impl<T: TriHashItem + PartialEq, S: Clone + BuildHasher, A: Allocator> PartialEq
    for TriHashMap<T, S, A>
{
    fn eq(&self, other: &Self) -> bool {
        // Implementing PartialEq for TriHashMap is tricky because TriHashMap is
        // not semantically like an IndexMap: two maps are equivalent even if
        // their items are in a different order. In other words, any permutation
        // of items is equivalent.
        //
        // We also can't sort the items because they're not necessarily Ord.
        //
        // So we write a custom equality check that checks that each key in one
        // map points to the same item as in the other map.

        if self.items.len() != other.items.len() {
            return false;
        }

        // Walk over all the items in the first map and check that they point to
        // the same item in the second map.
        for item in self.items.values() {
            let k1 = item.key1();
            let k2 = item.key2();
            let k3 = item.key3();

            // Check that the indexes are the same in the other map.
            let Some(other_ix1) = other.find1_index(&k1) else {
                return false;
            };
            let Some(other_ix2) = other.find2_index(&k2) else {
                return false;
            };
            let Some(other_ix3) = other.find3_index(&k3) else {
                return false;
            };

            if other_ix1 != other_ix2 || other_ix1 != other_ix3 {
                // All the keys were present but they didn't point to the same
                // item.
                return false;
            }

            // Check that the other map's item is the same as this map's
            // item. (This is what we use the `PartialEq` bound on T for.)
            //
            // Because we've checked that other_ix1, other_ix2 and other_ix3 are
            // Some, we know that it is valid and points to the expected item.
            let other_item = &other.items[other_ix1];
            if item != other_item {
                return false;
            }
        }

        true
    }
}

// The Eq bound on T ensures that the TriHashMap forms an equivalence class.
impl<T: TriHashItem + Eq, S: Clone + BuildHasher, A: Allocator> Eq
    for TriHashMap<T, S, A>
{
}

/// The `Extend` implementation overwrites duplicates. In the future, there will
/// also be an `extend_unique` method that will return an error.
impl<T: TriHashItem, S: Clone + BuildHasher, A: Allocator> Extend<T>
    for TriHashMap<T, S, A>
{
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            self.insert_overwrite(item);
        }
    }
}

fn detect_dup_or_insert<'a, A: Allocator>(
    item: Entry<'a, usize, AllocWrapper<A>>,
    duplicates: &mut BTreeSet<usize>,
) -> Option<VacantEntry<'a, usize, AllocWrapper<A>>> {
    match item {
        Entry::Vacant(slot) => Some(slot),
        Entry::Occupied(slot) => {
            duplicates.insert(*slot.get());
            None
        }
    }
}

impl<'a, T: TriHashItem, S: Clone + BuildHasher, A: Allocator> IntoIterator
    for &'a TriHashMap<T, S, A>
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: TriHashItem, S: Clone + BuildHasher, A: Allocator> IntoIterator
    for &'a mut TriHashMap<T, S, A>
{
    type Item = RefMut<'a, T, S>;
    type IntoIter = IterMut<'a, T, S, A>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: TriHashItem, S: Clone + BuildHasher, A: Allocator> IntoIterator
    for TriHashMap<T, S, A>
{
    type Item = T;
    type IntoIter = IntoIter<T, A>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self.items)
    }
}

/// The `FromIterator` implementation for `TriHashMap` overwrites duplicate
/// items.
impl<T: TriHashItem, S: Default + Clone + BuildHasher, A: Default + Allocator>
    FromIterator<T> for TriHashMap<T, S, A>
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut map = TriHashMap::default();
        for item in iter {
            map.insert_overwrite(item);
        }
        map
    }
}
