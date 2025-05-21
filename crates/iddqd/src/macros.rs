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
        fn upcast_key<'short, 'long: 'short>(
            long: Self::Key<'long>,
        ) -> Self::Key<'short> {
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
        fn upcast_key1<'short, 'long: 'short>(
            long: Self::K1<'long>,
        ) -> Self::K1<'short> {
            long
        }

        fn upcast_key2<'short, 'long: 'short>(
            long: Self::K2<'long>,
        ) -> Self::K2<'short> {
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
        fn upcast_key1<'short, 'long: 'short>(
            long: Self::K1<'long>,
        ) -> Self::K1<'short> {
            long
        }

        fn upcast_key2<'short, 'long: 'short>(
            long: Self::K2<'long>,
        ) -> Self::K2<'short> {
            long
        }

        fn upcast_key3<'short, 'long: 'short>(
            long: Self::K3<'long>,
        ) -> Self::K3<'short> {
            long
        }
    };
}
