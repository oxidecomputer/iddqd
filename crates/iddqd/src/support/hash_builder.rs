/// Default hasher for hash map types.
///
/// To disable this hasher, disable the `default-hasher` feature.
#[cfg(feature = "default-hasher")]
pub type DefaultHashBuilder = foldhash::fast::RandomState;

#[cfg(not(feature = "default-hasher"))]
mod dummy {
    use core::hash::{BuildHasher, Hasher};

    /// Dummy default hasher for hash map types.
    ///
    /// The `default-hasher` feature is currently disabled.
    #[derive(Clone, Copy, Debug)]
    pub enum DefaultHashBuilder {}

    impl BuildHasher for DefaultHashBuilder {
        type Hasher = Self;

        fn build_hasher(&self) -> Self::Hasher {
            match self {}
        }
    }

    impl Hasher for DefaultHashBuilder {
        fn write(&mut self, _bytes: &[u8]) {
            match self {}
        }

        fn finish(&self) -> u64 {
            match self {}
        }
    }
}

#[cfg(not(feature = "default-hasher"))]
pub use dummy::DefaultHashBuilder;
