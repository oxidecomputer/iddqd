use super::alloc::Allocator;
use flex_array::FlexArr;

/// An ordered map of items stored by integer index.
pub(crate) struct OrderedSet<T, A: Allocator> {
    // A dense vector for storage.
    items: FlexArr<T, A, usize>,
}

impl<T: Clone, A: Clone + Allocator> Clone for OrderedSet<T, A> {
    fn clone(&self) -> Self {
        // TODO: upstream this into flex_array
        let mut items = FlexArr::with_capacity_in(
            self.allocator().clone(),
            self.items.capacity(),
        )
        .expect("allocation succeeded");
        items.clone_from_slice(&self.items);
        Self { items }
    }
}

impl<T, A: Allocator + Default> Default for OrderedSet<T, A> {
    fn default() -> Self {
        Self::with_capacity_in(0, A::default())
    }
}

impl<T, A: Allocator> OrderedSet<T, A> {
    pub(crate) fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        Self {
            items: FlexArr::with_capacity_in(alloc, capacity)
                .expect("allocation succeeded"),
        }
    }

    #[inline]
    pub(crate) fn allocator(&self) -> &A {
        FlexArr::allocator(&self.items)
    }

    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        self.items.capacity()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    #[inline]
    pub(crate) fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }

    #[inline]
    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.items.get_mut(index)
    }

    #[inline]
    pub(crate) fn shift_remove(&mut self, index: usize) -> Option<T> {
        self.items.remove(index)
    }
}
