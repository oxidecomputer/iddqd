//! Macros for this crate.

/// Implement upcasts for [`IdOrdMap`] or [`IdHashMap`].
///
/// The maps in this crate require that the key types' lifetimes are covariant.
/// This macro assists with implementing this requirement.
///
/// The macro is optional, and these implementations can be written by hand as
/// well.
///
/// [`IdOrdMap`]: crate::IdOrdMap
/// [`IdHashMap`]: crate::IdHashMap
#[macro_export]
macro_rules! id_upcast {
    () => {
        #[inline]
        fn upcast_key<'short, 'long: 'short>(
            long: Self::Key<'long>,
        ) -> Self::Key<'short>
        where
            Self: 'long,
        {
            long
        }
    };
}

/// Implement upcasts for [`BiHashMap`].
///
/// The maps in this crate require that the key types' lifetimes are covariant.
/// This macro assists with implementing this requirement.
///
/// The macro is optional, and these implementations can be written by hand as
/// well.
///
/// [`BiHashMap`]: crate::BiHashMap
#[macro_export]
macro_rules! bi_upcast {
    () => {
        #[inline]
        fn upcast_key1<'short, 'long: 'short>(
            long: Self::K1<'long>,
        ) -> Self::K1<'short>
        where
            Self: 'long,
        {
            long
        }

        #[inline]
        fn upcast_key2<'short, 'long: 'short>(
            long: Self::K2<'long>,
        ) -> Self::K2<'short>
        where
            Self: 'long,
        {
            long
        }
    };
}

/// Implement upcasts for [`TriHashMap`].
///
/// The maps in this crate require that the key types' lifetimes are covariant.
/// This macro assists with implementing this requirement.
///
/// The macro is optional, and these implementations can be written by hand as
/// well.
///
/// [`TriHashMap`]: crate::TriHashMap
#[macro_export]
macro_rules! tri_upcast {
    () => {
        #[inline]
        fn upcast_key1<'short, 'long: 'short>(
            long: Self::K1<'long>,
        ) -> Self::K1<'short>
        where
            Self: 'long,
        {
            long
        }

        #[inline]
        fn upcast_key2<'short, 'long: 'short>(
            long: Self::K2<'long>,
        ) -> Self::K2<'short>
        where
            Self: 'long,
        {
            long
        }

        #[inline]
        fn upcast_key3<'short, 'long: 'short>(
            long: Self::K3<'long>,
        ) -> Self::K3<'short>
        where
            Self: 'long,
        {
            long
        }
    };
}

// Internal macro to implement diffs.
#[cfg(feature = "daft")]
macro_rules! impl_diff_ref_cast {
    ($self: ident, $diff_ty: ty, $key_method: ident, $get_method: ident, $contains_method: ident, $ref_cast_ty: ty) => {{
        let hasher = $self.before.hasher().clone();
        let alloc = $self.before.allocator().clone();
        let mut diff = <$diff_ty>::with_hasher_in(hasher, alloc);
        for before_item in $self.before {
            if let Some(after_item) =
                $self.after.$get_method(&before_item.$key_method())
            {
                diff.common.insert_overwrite(IdLeaf::new(
                    <$ref_cast_ty>::ref_cast(before_item),
                    <$ref_cast_ty>::ref_cast(after_item),
                ));
            } else {
                diff.removed
                    .insert_overwrite(<$ref_cast_ty>::ref_cast(before_item));
            }
        }
        for after_item in $self.after {
            if !$self.before.$contains_method(&after_item.$key_method()) {
                diff.added
                    .insert_overwrite(<$ref_cast_ty>::ref_cast(after_item));
            }
        }
        diff
    }};
}
