use super::{RefMut, tables::IdIndexMapTables};
use crate::{
    DefaultHashBuilder, IdHashItem,
    support::{
        alloc::{AllocWrapper, Allocator, Global},
        item_set::ItemSet,
        ordered_set::OrderedSet,
    },
};
use core::{hash::BuildHasher, iter::FusedIterator};

/// An iterator over the elements of a [`IdIndexMap`] by shared reference.
/// Created by [`IdIndexMap::iter`].
///
/// Items are yielded in insertion order.
///
/// [`IdIndexMap`]: crate::IdIndexMap
/// [`IdIndexMap::iter`]: crate::IdIndexMap::iter
#[derive(Clone, Debug, Default)]
pub struct Iter<'a, T: IdHashItem> {
    // TODO: Implement internal iterator structure
    _phantom: core::marker::PhantomData<&'a T>,
}

impl<'a, T: IdHashItem> Iter<'a, T> {
    pub(crate) fn new<A: Allocator>(items: &'a OrderedSet<T, A>) -> Self {
        // TODO: Implement
        todo!()
    }
}

impl<'a, T: IdHashItem> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // TODO: Implement
        todo!()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // TODO: Implement
        todo!()
    }
}

impl<'a, T: IdHashItem> DoubleEndedIterator for Iter<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        // TODO: Implement
        todo!()
    }
}

impl<T: IdHashItem> ExactSizeIterator for Iter<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        // TODO: Implement
        todo!()
    }
}

impl<T: IdHashItem> FusedIterator for Iter<'_, T> {}

/// An iterator over the elements of a [`IdIndexMap`] by mutable reference.
/// Created by [`IdIndexMap::iter_mut`].
///
/// This iterator returns [`RefMut`] instances.
///
/// Items are yielded in insertion order.
///
/// [`IdIndexMap`]: crate::IdIndexMap
/// [`IdIndexMap::iter_mut`]: crate::IdIndexMap::iter_mut
#[derive(Debug)]
pub struct IterMut<
    'a,
    T: IdHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    // TODO: Implement internal iterator structure
    _phantom: core::marker::PhantomData<(&'a mut T, S, A)>,
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher, A: Allocator>
    IterMut<'a, T, S, A>
{
    pub(super) fn new(
        tables: &'a IdIndexMapTables<S, A>,
        items: &'a mut ItemSet<T, A>,
    ) -> Self {
        // TODO: Implement
        todo!()
    }
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher, A: Allocator> Iterator
    for IterMut<'a, T, S, A>
{
    type Item = RefMut<'a, T, S>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // TODO: Implement
        todo!()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // TODO: Implement
        todo!()
    }
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher, A: Allocator>
    DoubleEndedIterator for IterMut<'a, T, S, A>
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        // TODO: Implement
        todo!()
    }
}

impl<T: IdHashItem, S: Clone + BuildHasher, A: Allocator> ExactSizeIterator
    for IterMut<'_, T, S, A>
{
    #[inline]
    fn len(&self) -> usize {
        // TODO: Implement
        todo!()
    }
}

impl<T: IdHashItem, S: Clone + BuildHasher, A: Allocator> FusedIterator
    for IterMut<'_, T, S, A>
{
}

/// An iterator over the elements of a [`IdIndexMap`] by ownership. Created by
/// [`IdIndexMap::into_iter`].
///
/// Items are yielded in insertion order.
///
/// [`IdIndexMap`]: crate::IdIndexMap
/// [`IdIndexMap::into_iter`]: crate::IdIndexMap::into_iter
#[derive(Debug)]
pub struct IntoIter<T: IdHashItem, A: Allocator = Global> {
    // TODO: Implement internal iterator structure
    _phantom: core::marker::PhantomData<(T, A)>,
}

impl<T: IdHashItem, A: Allocator> IntoIter<T, A> {
    pub(crate) fn new(items: ItemSet<T, A>) -> Self {
        // TODO: Implement
        todo!()
    }
}

impl<T: IdHashItem, A: Allocator> Iterator for IntoIter<T, A> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // TODO: Implement
        todo!()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // TODO: Implement
        todo!()
    }
}

impl<T: IdHashItem, A: Allocator> DoubleEndedIterator for IntoIter<T, A> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        // TODO: Implement
        todo!()
    }
}

impl<T: IdHashItem, A: Allocator> ExactSizeIterator for IntoIter<T, A> {
    #[inline]
    fn len(&self) -> usize {
        // TODO: Implement
        todo!()
    }
}

impl<T: IdHashItem, A: Allocator> FusedIterator for IntoIter<T, A> {}

/// An iterator over the keys of a [`IdIndexMap`] by shared reference.
#[derive(Clone, Debug)]
pub struct Keys<'a, T: IdHashItem> {
    inner: Iter<'a, T>,
}

impl<'a, T: IdHashItem> Keys<'a, T> {
    pub(crate) fn new(iter: Iter<'a, T>) -> Self {
        Self { inner: iter }
    }
}

impl<'a, T: IdHashItem> Iterator for Keys<'a, T> {
    type Item = T::Key<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| item.key())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, T: IdHashItem> DoubleEndedIterator for Keys<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|item| item.key())
    }
}

impl<T: IdHashItem> ExactSizeIterator for Keys<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<T: IdHashItem> FusedIterator for Keys<'_, T> {}

/// An iterator over the values of a [`IdIndexMap`] by shared reference.
#[derive(Clone, Debug)]
pub struct Values<'a, T: IdHashItem> {
    inner: Iter<'a, T>,
}

impl<'a, T: IdHashItem> Values<'a, T> {
    pub(crate) fn new(iter: Iter<'a, T>) -> Self {
        Self { inner: iter }
    }
}

impl<'a, T: IdHashItem> Iterator for Values<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, T: IdHashItem> DoubleEndedIterator for Values<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

impl<T: IdHashItem> ExactSizeIterator for Values<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<T: IdHashItem> FusedIterator for Values<'_, T> {}

/// An iterator over the values of a [`IdIndexMap`] by mutable reference.
#[derive(Debug)]
pub struct ValuesMut<
    'a,
    T: IdHashItem,
    S = DefaultHashBuilder,
    A: Allocator = Global,
> {
    inner: IterMut<'a, T, S, A>,
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher, A: Allocator>
    ValuesMut<'a, T, S, A>
{
    pub(crate) fn new(iter: IterMut<'a, T, S, A>) -> Self {
        Self { inner: iter }
    }
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher, A: Allocator> Iterator
    for ValuesMut<'a, T, S, A>
{
    type Item = RefMut<'a, T, S>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, T: IdHashItem, S: Clone + BuildHasher, A: Allocator>
    DoubleEndedIterator for ValuesMut<'a, T, S, A>
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

impl<T: IdHashItem, S: Clone + BuildHasher, A: Allocator> ExactSizeIterator
    for ValuesMut<'_, T, S, A>
{
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<T: IdHashItem, S: Clone + BuildHasher, A: Allocator> FusedIterator
    for ValuesMut<'_, T, S, A>
{
}

/// An iterator over the indices and values of a [`IdIndexMap`] by shared reference.
#[derive(Clone, Debug)]
pub struct Enumerate<'a, T: IdHashItem> {
    inner: Iter<'a, T>,
    index: usize,
}

impl<'a, T: IdHashItem> Enumerate<'a, T> {
    pub(crate) fn new(iter: Iter<'a, T>) -> Self {
        Self { inner: iter, index: 0 }
    }
}

impl<'a, T: IdHashItem> Iterator for Enumerate<'a, T> {
    type Item = (usize, &'a T);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|item| {
            let index = self.index;
            self.index += 1;
            (index, item)
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T: IdHashItem> ExactSizeIterator for Enumerate<'_, T> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<T: IdHashItem> FusedIterator for Enumerate<'_, T> {}
