/// Default hasher for hash map types.
#[cfg(feature = "default-hasher")]
pub type DefaultHashBuilder = foldhash::fast::RandomState;

#[cfg(not(feature = "default-hasher"))]
mod dummy {
    use core::hash::{BuildHasher, Hasher};

    /// Dummy default hasher for hash map types.
    #[derive(Clone, Copy, Debug)]
    pub enum DefaultHashBuilder {}

    impl BuildHasher for DefaultHashBuilder {
        type Hasher = Self;

        fn build_hasher(&self) -> Self::Hasher {
            match self {
                _ => unreachable!(),
            }
        }
    }

    impl Hasher for DefaultHashBuilder {
        fn write(&mut self, _bytes: &[u8]) {}

        fn finish(&self) -> u64 {
            0
        }
    }
}

#[cfg(not(feature = "default-hasher"))]
pub use dummy::DefaultHashBuilder;
