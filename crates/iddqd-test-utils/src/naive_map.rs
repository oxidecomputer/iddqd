use crate::test_item::TestItem;
use iddqd::errors::DuplicateItem;

/// A naive, inefficient map that acts as an oracle for property-based tests.
///
/// This map is stored as a vector without internal indexes, and performs linear
/// scans.
#[derive(Debug)]
pub struct NaiveMap {
    items: Vec<TestItem>,
    unique_constraint: UniqueConstraint,
}

impl NaiveMap {
    pub fn new_key1() -> Self {
        Self { items: Vec::new(), unique_constraint: UniqueConstraint::Key1 }
    }

    // Will use in the future.
    pub fn new_key12() -> Self {
        Self { items: Vec::new(), unique_constraint: UniqueConstraint::Key12 }
    }

    pub fn new_key123() -> Self {
        Self { items: Vec::new(), unique_constraint: UniqueConstraint::Key123 }
    }

    pub fn insert_unique(
        &mut self,
        item: TestItem,
    ) -> Result<(), DuplicateItem<TestItem, &TestItem>> {
        // Cannot store the duplicates directly here because of borrow checker
        // issues. Instead, we store indexes and then map them to items.
        let dup_indexes = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                self.unique_constraint.matches(&item, e).then_some(i)
            })
            .collect::<Vec<_>>();

        if dup_indexes.is_empty() {
            self.items.push(item);
            Ok(())
        } else {
            Err(DuplicateItem::__internal_new(
                item,
                dup_indexes.iter().map(|&i| &self.items[i]).collect(),
            ))
        }
    }

    pub fn insert_overwrite(&mut self, item: TestItem) -> Vec<TestItem> {
        let dup_indexes = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                self.unique_constraint.matches(&item, e).then_some(i)
            })
            .collect::<Vec<_>>();
        let mut dups = Vec::new();

        // dup_indexes is in sorted order -- remove items in that order to
        // handle shifting indexes. (There are more efficient ways to do this.
        // But this is a model, not the system under test, so the goal here is
        // to be clear more than to be efficient.)
        for i in dup_indexes.iter().rev() {
            dups.push(self.items.remove(*i));
        }

        // Now we can push the new item.
        self.items.push(item);
        dups
    }

    pub fn get1(&self, key1: u8) -> Option<&TestItem> {
        self.items.iter().find(|e| e.key1 == key1)
    }

    pub fn get2(&self, key2: char) -> Option<&TestItem> {
        self.items.iter().find(|e| e.key2 == key2)
    }

    pub fn get3(&self, key3: &str) -> Option<&TestItem> {
        self.items.iter().find(|e| e.key3 == key3)
    }

    pub fn remove1(&mut self, key1: u8) -> Option<TestItem> {
        let index = self.items.iter().position(|e| e.key1 == key1)?;
        Some(self.items.remove(index))
    }

    pub fn remove2(&mut self, key2: char) -> Option<TestItem> {
        let index = self.items.iter().position(|e| e.key2 == key2)?;
        Some(self.items.remove(index))
    }

    pub fn remove3(&mut self, key3: &str) -> Option<TestItem> {
        let index = self.items.iter().position(|e| e.key3 == key3)?;
        Some(self.items.remove(index))
    }

    /// Returns the item whose two keys both match, if any.
    ///
    /// Mirrors `BiHashMap::get_unique`: a hit requires `key1` *and* `key2` to
    /// match the same item.
    pub fn get_unique12(&self, key1: u8, key2: char) -> Option<&TestItem> {
        self.items.iter().find(|e| e.key1 == key1 && e.key2 == key2)
    }

    /// Mutable variant of [`Self::get_unique12`].
    pub fn get_mut_unique12(
        &mut self,
        key1: u8,
        key2: char,
    ) -> Option<&mut TestItem> {
        self.items.iter_mut().find(|e| e.key1 == key1 && e.key2 == key2)
    }

    /// Removes and returns the item whose two keys both match, if any.
    pub fn remove_unique12(
        &mut self,
        key1: u8,
        key2: char,
    ) -> Option<TestItem> {
        let index =
            self.items.iter().position(|e| e.key1 == key1 && e.key2 == key2)?;
        Some(self.items.remove(index))
    }

    /// Removes and returns every item covered by a `BiHashMap` entry keyed on
    /// `(key1, key2)`, i.e., every item matching `key1` *or* `key2`.
    ///
    /// Mirrors `bi_hash_map::OccupiedEntry::remove`. An empty result
    /// corresponds to a vacant entry.
    pub fn entry_remove12(&mut self, key1: u8, key2: char) -> Vec<TestItem> {
        let indexes = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                (e.key1 == key1 || e.key2 == key2).then_some(i)
            })
            .collect::<Vec<_>>();

        // Remove in reverse so earlier indexes stay valid as items shift.
        indexes.iter().rev().map(|&i| self.items.remove(i)).collect()
    }

    /// Returns the item whose three keys all match, if any.
    ///
    /// Mirrors `TriHashMap::get_unique`: a hit requires `key1`, `key2`, *and*
    /// `key3` to match the same item.
    pub fn get_unique123(
        &self,
        key1: u8,
        key2: char,
        key3: &str,
    ) -> Option<&TestItem> {
        self.items
            .iter()
            .find(|e| e.key1 == key1 && e.key2 == key2 && e.key3 == key3)
    }

    /// Mutable variant of [`Self::get_unique123`].
    pub fn get_mut_unique123(
        &mut self,
        key1: u8,
        key2: char,
        key3: &str,
    ) -> Option<&mut TestItem> {
        self.items
            .iter_mut()
            .find(|e| e.key1 == key1 && e.key2 == key2 && e.key3 == key3)
    }

    /// Removes and returns the item whose three keys all match, if any.
    pub fn remove_unique123(
        &mut self,
        key1: u8,
        key2: char,
        key3: &str,
    ) -> Option<TestItem> {
        let index = self
            .items
            .iter()
            .position(|e| e.key1 == key1 && e.key2 == key2 && e.key3 == key3)?;
        Some(self.items.remove(index))
    }

    pub fn items(&self) -> &[TestItem] {
        &self.items
    }

    /// Removes and returns every item covered by a `TriHashMap` entry keyed on
    /// `(key1, key2, key3)`, i.e. every distinct item matching `key1`, `key2`,
    /// or `key3`.
    ///
    /// Mirrors `tri_hash_map::OccupiedEntry::remove`. The returned items are in
    /// first-key-hit order.
    pub fn entry_remove123(
        &mut self,
        key1: u8,
        key2: char,
        key3: &str,
    ) -> Vec<TestItem> {
        fn push_unique(indexes: &mut Vec<usize>, index: Option<usize>) {
            if let Some(index) = index {
                if !indexes.contains(&index) {
                    indexes.push(index);
                }
            }
        }

        let mut key_order = Vec::new();

        push_unique(
            &mut key_order,
            self.items.iter().position(|e| e.key1 == key1),
        );
        push_unique(
            &mut key_order,
            self.items.iter().position(|e| e.key2 == key2),
        );
        push_unique(
            &mut key_order,
            self.items.iter().position(|e| e.key3 == key3),
        );

        let mut remove_order = key_order.clone();
        remove_order.sort_unstable_by(|a, b| b.cmp(a));

        let mut removed = Vec::with_capacity(remove_order.len());
        for index in remove_order {
            removed.push((index, self.items.remove(index)));
        }

        key_order
            .into_iter()
            .map(|index| {
                let removed_index = removed
                    .iter()
                    .position(|(removed_index, _)| *removed_index == index)
                    .expect("index was removed");
                removed.swap_remove(removed_index).1
            })
            .collect()
    }

    /// Mirrors the test harness behavior for
    /// `TriHashMap::entry(...).Occupied(...).insert(...)`.
    ///
    /// Returns `None` for a vacant entry and does not insert. Returns
    /// `Some(removed)` for an occupied entry, where `removed` is in
    /// first-key-hit order.
    pub fn entry_insert_overwrite123(
        &mut self,
        item: TestItem,
    ) -> Option<Vec<TestItem>> {
        let occupied = self.get1(item.key1).is_some()
            || self.get2(item.key2).is_some()
            || self.get3(&item.key3).is_some();

        if !occupied {
            return None;
        }

        let removed = self.entry_remove123(item.key1, item.key2, &item.key3);
        self.items.push(item);
        Some(removed)
    }

    pub fn iter(&self) -> impl Iterator<Item = &TestItem> {
        self.items.iter()
    }

    pub fn first(&self) -> Option<&TestItem> {
        self.items.iter().min_by_key(|e| e.key1)
    }

    pub fn last(&self) -> Option<&TestItem> {
        self.items.iter().max_by_key(|e| e.key1)
    }

    pub fn pop_first(&mut self) -> Option<TestItem> {
        if self.items.is_empty() {
            return None;
        }
        let index = self
            .items
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| e.key1)
            .map(|(i, _)| i)?;
        Some(self.items.remove(index))
    }

    pub fn pop_last(&mut self) -> Option<TestItem> {
        if self.items.is_empty() {
            return None;
        }
        let index = self
            .items
            .iter()
            .enumerate()
            .max_by_key(|(_, e)| e.key1)
            .map(|(i, _)| i)?;
        Some(self.items.remove(index))
    }

    pub fn first_mut(&mut self) -> Option<&mut TestItem> {
        if self.items.is_empty() {
            return None;
        }
        let index = self
            .items
            .iter()
            .enumerate()
            .min_by_key(|(_, e)| e.key1)
            .map(|(i, _)| i)?;
        Some(&mut self.items[index])
    }

    pub fn last_mut(&mut self) -> Option<&mut TestItem> {
        if self.items.is_empty() {
            return None;
        }
        let index = self
            .items
            .iter()
            .enumerate()
            .max_by_key(|(_, e)| e.key1)
            .map(|(i, _)| i)?;
        Some(&mut self.items[index])
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&mut TestItem) -> bool,
    {
        // Sort items by key1 to match IdOrdMap iteration order
        self.items.sort_by_key(|e| e.key1);

        // Retain items matching the predicate
        self.items.retain_mut(f);
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn extend<I: IntoIterator<Item = TestItem>>(&mut self, iter: I) {
        // Mirrors `Extend` on the real maps: each item overwrites any
        // pre-existing duplicates under the active uniqueness constraint.
        for item in iter {
            self.insert_overwrite(item);
        }
    }
}

/// Which keys to check uniqueness against.
#[derive(Clone, Copy, Debug)]
enum UniqueConstraint {
    Key1,
    Key12,
    Key123,
}

impl UniqueConstraint {
    fn matches(&self, item: &TestItem, other: &TestItem) -> bool {
        match self {
            UniqueConstraint::Key1 => item.key1 == other.key1,
            UniqueConstraint::Key12 => {
                item.key1 == other.key1 || item.key2 == other.key2
            }
            UniqueConstraint::Key123 => {
                item.key1 == other.key1
                    || item.key2 == other.key2
                    || item.key3 == other.key3
            }
        }
    }
}
