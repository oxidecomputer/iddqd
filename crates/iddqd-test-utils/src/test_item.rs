use iddqd::{
    BiHashItem, BiHashMap, IdHashItem, IdHashMap, TriHashItem, TriHashMap,
    bi_hash_map, bi_upcast,
    errors::DuplicateItem,
    id_hash_map, id_upcast,
    internal::{ValidateCompact, ValidationError},
    tri_hash_map, tri_upcast,
};
#[cfg(feature = "std")]
use iddqd::{IdOrdItem, IdOrdMap, id_ord_map};
use proptest::{prelude::*, sample::SizeRange};
use std::cell::Cell;
use test_strategy::Arbitrary;

thread_local! {
    static WITHOUT_CHAOS: Cell<bool> = const { Cell::new(false) };
}

/// Temporarily disable chaos testing.
pub fn without_chaos<F, T>(f: F)
where
    F: FnOnce() -> T,
{
    let guard = ChaosGuard::new();
    f();
    // Explicitly drop the guard to ensure that the chaos flag is reset.
    drop(guard);
}

struct ChaosGuard {}

impl ChaosGuard {
    fn new() -> Self {
        WITHOUT_CHAOS.set(true);
        Self {}
    }
}

impl Drop for ChaosGuard {
    fn drop(&mut self) {
        WITHOUT_CHAOS.set(false);
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Arbitrary)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestItem {
    pub key1: u8,
    pub key2: char,
    pub key3: String,
    pub value: String,
    #[strategy(Just(TestChaos::default()))]
    pub chaos: TestChaos,
}

impl TestItem {
    pub fn new(
        key1: u8,
        key2: char,
        key3: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        Self {
            key1,
            key2,
            key3: key3.into(),
            value: value.into(),
            chaos: TestChaos::default(),
        }
    }

    pub fn with_key1_chaos(self, chaos: KeyChaos) -> Self {
        let chaos = TestChaos { key1_chaos: chaos, ..self.chaos };
        Self { chaos, ..self }
    }
}

impl PartialEq<&TestItem> for TestItem {
    fn eq(&self, other: &&TestItem) -> bool {
        self.key1 == other.key1
            && self.key2 == other.key2
            && self.key3 == other.key3
            && self.value == other.value
            && self.chaos == other.chaos
    }
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TestChaos {
    pub key1_chaos: KeyChaos,
    pub key2_chaos: KeyChaos,
    pub key3_chaos: KeyChaos,
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyChaos {
    pub eq: Option<ChaosEq>,
    pub ord: Option<ChaosOrd>,
}

impl KeyChaos {
    pub fn with_eq(self, chaos: ChaosEq) -> Self {
        Self { eq: Some(chaos), ..self }
    }

    pub fn with_ord(self, chaos: ChaosOrd) -> Self {
        Self { ord: Some(chaos), ..self }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChaosEq {
    Always,
    Never,
    FlipFlop(Cell<bool>),
}

impl ChaosEq {
    pub fn all_variants() -> [Self; 3] {
        [Self::Always, Self::Never, Self::FlipFlop(Cell::new(false))]
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChaosOrd {
    AlwaysLess,
    AlwaysGreater,
    AlwaysEq,
    FlipFlop(Cell<bool>),
}

impl ChaosOrd {
    pub fn all_variants() -> [Self; 4] {
        [
            Self::AlwaysLess,
            Self::AlwaysGreater,
            Self::AlwaysEq,
            Self::FlipFlop(Cell::new(false)),
        ]
    }
}

macro_rules! impl_test_key_traits {
    ($name:ty) => {
        impl std::hash::Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                // TODO: add chaos testing for hashes
                self.key.hash(state);
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                if WITHOUT_CHAOS.get() {
                    return self.key == other.key;
                }
                match self.chaos.eq {
                    Some(ChaosEq::Always) => true,
                    Some(ChaosEq::Never) => false,
                    Some(ChaosEq::FlipFlop(ref cell)) => {
                        let value = cell.get();
                        cell.set(!value);
                        value
                    }
                    None => self.key == other.key,
                }
            }
        }

        impl Eq for $name {}

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                if WITHOUT_CHAOS.get() {
                    return self.key.cmp(&other.key);
                }
                match self.chaos.ord {
                    Some(ChaosOrd::AlwaysLess) => std::cmp::Ordering::Less,
                    Some(ChaosOrd::AlwaysGreater) => {
                        std::cmp::Ordering::Greater
                    }
                    Some(ChaosOrd::AlwaysEq) => std::cmp::Ordering::Equal,
                    Some(ChaosOrd::FlipFlop(ref cell)) => {
                        let value = cell.get();
                        cell.set(!value);
                        if value {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Greater
                        }
                    }
                    None => self.key.cmp(&other.key),
                }
            }
        }
    };
}

#[derive(Clone, Debug)]
pub struct TestKey1<'a> {
    // We use u8 since there can only be 256 values, increasing the
    // likelihood of collisions in proptests.
    //
    // A bit weird to return a reference to a u8, but this makes sure
    // reference-based keys work properly.
    key: &'a u8,
    chaos: KeyChaos,
}

impl<'a> TestKey1<'a> {
    pub fn new(key: &'a u8) -> Self {
        Self { key, chaos: KeyChaos::default() }
    }

    pub fn with_chaos(self, chaos: KeyChaos) -> Self {
        Self { chaos, ..self }
    }
}

impl_test_key_traits!(TestKey1<'_>);

#[derive(Clone, Debug)]
pub struct TestKey2 {
    // char is chosen because the Arbitrary impl for it is biased towards
    // ASCII, increasing the likelihood of collisions.
    key: char,
    chaos: KeyChaos,
}

impl TestKey2 {
    pub fn new(key: char) -> Self {
        Self { key, chaos: KeyChaos::default() }
    }

    pub fn with_chaos(self, chaos: KeyChaos) -> Self {
        Self { chaos, ..self }
    }
}

impl_test_key_traits!(TestKey2);

#[derive(Clone, Debug)]
pub struct TestKey3<'a> {
    // &str is a generally open-ended type that probably won't have many
    // collisions.
    key: &'a str,
    chaos: KeyChaos,
}

impl<'a> TestKey3<'a> {
    pub fn new(key: &'a str) -> Self {
        Self { key, chaos: KeyChaos::default() }
    }

    pub fn with_chaos(self, chaos: KeyChaos) -> Self {
        Self { chaos, ..self }
    }
}

impl_test_key_traits!(TestKey3<'_>);

impl IdHashItem for TestItem {
    type Key<'a> = TestKey1<'a>;

    fn key(&self) -> Self::Key<'_> {
        TestKey1::new(&self.key1)
    }

    id_upcast!();
}

#[cfg(feature = "std")]
impl IdOrdItem for TestItem {
    // A bit weird to return a reference to a u8, but this makes sure
    // reference-based keys work properly.
    type Key<'a> = TestKey1<'a>;

    fn key(&self) -> Self::Key<'_> {
        TestKey1::new(&self.key1).with_chaos(self.chaos.key1_chaos.clone())
    }

    id_upcast!();
}

impl BiHashItem for TestItem {
    type K1<'a> = TestKey1<'a>;
    type K2<'a> = TestKey2;

    fn key1(&self) -> Self::K1<'_> {
        TestKey1::new(&self.key1).with_chaos(self.chaos.key1_chaos.clone())
    }

    fn key2(&self) -> Self::K2<'_> {
        TestKey2::new(self.key2).with_chaos(self.chaos.key2_chaos.clone())
    }

    bi_upcast!();
}

impl TriHashItem for TestItem {
    type K1<'a> = TestKey1<'a>;
    type K2<'a> = TestKey2;
    type K3<'a> = TestKey3<'a>;

    fn key1(&self) -> Self::K1<'_> {
        TestKey1::new(&self.key1).with_chaos(self.chaos.key1_chaos.clone())
    }

    fn key2(&self) -> Self::K2<'_> {
        TestKey2::new(self.key2).with_chaos(self.chaos.key2_chaos.clone())
    }

    fn key3(&self) -> Self::K3<'_> {
        TestKey3::new(&self.key3).with_chaos(self.chaos.key3_chaos.clone())
    }

    tri_upcast!();
}

pub enum MapKind {
    Ord,
    Hash,
}

/// Represents a map of `TestEntry` values. Used for generic tests and assertions.
pub trait TestItemMap: Clone {
    type RefMut<'a>: IntoRef<'a>
    where
        Self: 'a;
    type Iter<'a>: Iterator<Item = &'a TestItem>
    where
        Self: 'a;
    type IterMut<'a>: Iterator<Item = Self::RefMut<'a>>
    where
        Self: 'a;
    type IntoIter: Iterator<Item = TestItem>;

    fn map_kind() -> MapKind;
    fn new() -> Self;
    fn validate_(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError>;
    fn insert_unique(
        &mut self,
        value: TestItem,
    ) -> Result<(), DuplicateItem<TestItem, &TestItem>>;
    fn iter(&self) -> Self::Iter<'_>;
    fn iter_mut(&mut self) -> Self::IterMut<'_>;
    fn into_iter(self) -> Self::IntoIter;
}

impl TestItemMap for BiHashMap<TestItem> {
    type RefMut<'a> = bi_hash_map::RefMut<'a, TestItem>;
    type Iter<'a> = bi_hash_map::Iter<'a, TestItem>;
    type IterMut<'a> = bi_hash_map::IterMut<'a, TestItem>;
    type IntoIter = bi_hash_map::IntoIter<TestItem>;

    fn map_kind() -> MapKind {
        MapKind::Hash
    }

    fn new() -> Self {
        BiHashMap::new()
    }

    fn validate_(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        self.validate(compactness)
    }

    fn insert_unique(
        &mut self,
        value: TestItem,
    ) -> Result<(), DuplicateItem<TestItem, &TestItem>> {
        self.insert_unique(value)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.iter_mut()
    }

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(self)
    }
}

impl TestItemMap for IdHashMap<TestItem> {
    type RefMut<'a> = id_hash_map::RefMut<'a, TestItem>;
    type Iter<'a> = id_hash_map::Iter<'a, TestItem>;
    type IterMut<'a> = id_hash_map::IterMut<'a, TestItem>;
    type IntoIter = id_hash_map::IntoIter<TestItem>;

    fn map_kind() -> MapKind {
        MapKind::Hash
    }

    fn new() -> Self {
        IdHashMap::new()
    }

    fn validate_(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        self.validate(compactness)
    }

    fn insert_unique(
        &mut self,
        value: TestItem,
    ) -> Result<(), DuplicateItem<TestItem, &TestItem>> {
        self.insert_unique(value)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.iter_mut()
    }

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(self)
    }
}

#[cfg(feature = "std")]
impl TestItemMap for IdOrdMap<TestItem> {
    type RefMut<'a> = id_ord_map::RefMut<'a, TestItem>;
    type Iter<'a> = id_ord_map::Iter<'a, TestItem>;
    type IterMut<'a> = id_ord_map::IterMut<'a, TestItem>;
    type IntoIter = id_ord_map::IntoIter<TestItem>;

    fn map_kind() -> MapKind {
        MapKind::Ord
    }

    fn new() -> Self {
        IdOrdMap::new()
    }

    fn validate_(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        self.validate(compactness, iddqd::internal::ValidateChaos::No)
    }

    fn insert_unique(
        &mut self,
        value: TestItem,
    ) -> Result<(), DuplicateItem<TestItem, &TestItem>> {
        self.insert_unique(value)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.iter_mut()
    }

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(self)
    }
}

impl TestItemMap for TriHashMap<TestItem> {
    type RefMut<'a> = tri_hash_map::RefMut<'a, TestItem>;
    type Iter<'a> = tri_hash_map::Iter<'a, TestItem>;
    type IterMut<'a> = tri_hash_map::IterMut<'a, TestItem>;
    type IntoIter = tri_hash_map::IntoIter<TestItem>;

    fn map_kind() -> MapKind {
        MapKind::Hash
    }

    fn new() -> Self {
        TriHashMap::new()
    }

    fn validate_(
        &self,
        compactness: ValidateCompact,
    ) -> Result<(), ValidationError> {
        self.validate(compactness)
    }

    fn insert_unique(
        &mut self,
        value: TestItem,
    ) -> Result<(), DuplicateItem<TestItem, &TestItem>> {
        self.insert_unique(value)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.iter_mut()
    }

    fn into_iter(self) -> Self::IntoIter {
        IntoIterator::into_iter(self)
    }
}

pub trait IntoRef<'a> {
    fn into_ref(self) -> &'a TestItem;
}

impl<'a> IntoRef<'a> for bi_hash_map::RefMut<'a, TestItem> {
    fn into_ref(self) -> &'a TestItem {
        self.into_ref()
    }
}

impl<'a> IntoRef<'a> for id_hash_map::RefMut<'a, TestItem> {
    fn into_ref(self) -> &'a TestItem {
        self.into_ref()
    }
}

#[cfg(feature = "std")]
impl<'a> IntoRef<'a> for id_ord_map::RefMut<'a, TestItem> {
    fn into_ref(self) -> &'a TestItem {
        self.into_ref()
    }
}

impl<'a> IntoRef<'a> for tri_hash_map::RefMut<'a, TestItem> {
    fn into_ref(self) -> &'a TestItem {
        self.into_ref()
    }
}

pub fn assert_iter_eq<M: TestItemMap>(mut map: M, items: Vec<&TestItem>) {
    let mut iter = map.iter().collect::<Vec<_>>();
    iter.sort_by_key(|e| e.key1);
    assert_eq!(iter, items, ".iter() items match naive ones");

    let mut iter_mut = map.iter_mut().map(|v| v.into_ref()).collect::<Vec<_>>();
    iter_mut.sort_by_key(|e| e.key1);
    assert_eq!(iter_mut, items, ".iter_mut() items match naive ones");

    let mut into_iter = map.clone().into_iter().collect::<Vec<_>>();
    into_iter.sort_by_key(|e| e.key1);
    assert_eq!(into_iter, items, ".into_iter() items match naive ones");
}

// Returns a pair of permutations of a set of unique items (unique to a given
// map).
pub fn test_item_permutation_strategy<M: TestItemMap>(
    size: impl Into<SizeRange>,
) -> impl Strategy<Value = (Vec<TestItem>, Vec<TestItem>)> {
    prop::collection::vec(any::<TestItem>(), size.into()).prop_perturb(
        |v, mut rng| {
            // It is possible (likely even) that the input vector has
            // duplicates. How can we remove them? The easiest way is to use
            // the logic that already exists to check for duplicates. Insert
            // all the items one by one, then get the list.
            let mut map = M::new();
            for item in v {
                // The error case here is expected -- we're actively de-duping
                // items right now.
                _ = map.insert_unique(item);
            }
            let set: Vec<_> = map.into_iter().collect();

            // Now shuffle the items. This is a simple Fisher-Yates shuffle
            // (Durstenfeld variant, low to high).
            let mut set2 = set.clone();
            if set.len() < 2 {
                return (set, set2);
            }
            for i in 0..set2.len() - 2 {
                let j = rng.gen_range(i..set2.len());
                set2.swap(i, j);
            }

            (set, set2)
        },
    )
}
