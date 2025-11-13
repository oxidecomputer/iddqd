# Changelog

## [Unreleased]

### Added

- Capacity management methods for all map types:
  - `reserve(&mut self, additional: usize)` reserves capacity for at least `additional` more elements.
  - `shrink_to_fit(&mut self)` shrinks capacity to fit the current length.
  - `shrink_to(&mut self, min_capacity: usize)` shrinks capacity to at least `min_capacity`.
  - `try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError>`: fallible capacity reservation for hash maps (`IdHashMap`, `BiHashMap`, `TriHashMap`).
- New `TryReserveError` type in the `errors` module for reporting allocation failures.

#### Notes

- For `IdOrdMap`, the reserve and shrink methods only affect item storage. The internal `BTreeSet` used for key-to-item mapping does not support capacity control.
- `IdOrdMap` does not provide `try_reserve`, since the underlying `BTreeMap` does not expose fallible reservation operations.

### Changed

The `Extend` implementations now pre-reserve capacity based on the iterator's `size_hint`.

## [0.3.16] - 2025-11-09

### Added

- `clear` methods for all map types to remove all items from the map.
- Optionally, serialize ID maps as maps (JSON objects) rather than sequences (JSON arrays):
  - New `IdHashMapAsMap`, `BiHashMapAsMap`, `TriHashMapAsMap`, and `IdOrdMapAsMap` marker types to use with `#[serde(with = ...)]`.
  - The default deserializer for each map now accepts both maps and sequences.

### Changed

- Documentation improvements for serde implementations.

## [0.3.15] - 2025-11-08

### Added

- `retain` methods that allow filtering items in place based on a predicate.
- `IdOrdMap::first`, `first_entry`, `last`, `last_entry`, `pop_first`, and `pop_last` methods for accessing entries at the beginning and end of the map.
- `BiHashMap::with_hasher` and `TriHashMap::with_hasher` are now `const fn`.

### Changed

- Reduced memory footprint for `BiHashMap` and `TriHashMap`.

## [0.3.14] - 2025-09-29

### Fixed

Replaced obsolete `doc_auto_cfg` with `doc_cfg`, to fix Rust nightly builds with the `doc_cfg` flag enabled.

## [0.3.13] - 2025-09-14

### Changed

- iddqd now depends on `serde_core` rather than `serde`. This allows iddqd's compilation to be parallelized with `serde_derive`.
- Internal dependency updates: `foldhash` updated to 0.2, and `hashbrown` updated to 0.16.

## [0.3.12] - 2025-09-12

### Added

The following methods are now `const fn`:

- `IdOrdMap::new`
- `IdHashMap::with_hasher`

## [0.3.11] - 2025-08-14

### Fixed

The type definitions for `IdHashMap`, `BiHashMap`, `TriHashMap`, and `IdOrdMap` no longer require `IdHashItem`, `BiHashItem`, `TriHashItem`, and `IdOrdItem`, respectively. This matches the standard library's `HashMap` and `BTreeMap` type definitions which don't require `Hash + Eq` or `Ord` bounds.

Thanks to [aatifsyed](https://github.com/aatifsyed) for your first contribution!

## [0.3.10] - 2025-08-12

### Fixed

`id_ord_map::RefMut`'s `reborrow` method now works if the underlying `IdOrdItem` is non-`'static`.

## [0.3.9] - 2025-07-21

### Added

- For the optional `daft` feature, the map `Diff` types now implement `fmt::Debug`.

### Miscellaneous

- Several documentation fixes.

## [0.3.8] - 2025-06-22

### Added

- New `proptest` feature adds strategy and `Arbitrary` implementations for map types.

## [0.3.7] - 2025-06-11

### Fixed

- iddqd is now compatible with schemars's `preserve_order` feature. Thanks [Sh3Rm4n](https://github.com/Sh3Rm4n) for your first contribution!

## [0.3.6] - 2025-06-06

### Changed

- Relaxed `Debug` requirement to only require that `T::Key<'a>: fmt::Debug`, not `for<'k> T::Key<'k>: fmt::Debug`. This allows items with borrowed data to compile in more cases.
- Relaxed `Hash` requirement for `IdOrdMap` `get_mut` and related APIs in a similar fashion.

## [0.3.5] - 2025-06-05

### Added

- New feature `schemars08` adds support for generating JSON schemas.

## [0.3.4] - 2025-06-03

### Added

- New macros `id_hash_map`, `bi_hash_map`, `tri_hash_map`, and `id_ord_map` allow easy construction of literal macros. These macros use `insert_unique`, so they panic if duplicate keys are encountered.

### Changed

- The `id_upcast`, `bi_upcast` and `tri_upcast` macros now have a `Self: 'long` bound, allowing them to be used for non-`'static` items.
- Minimized dependency list, removing the dependency on `derive-where`, `debug-ignore`, and serde's `derive` feature. iddqd no longer depends on any proc macros.

## [0.3.3] - 2025-05-27

### Added

- A lot of new documentation. Most functions now have doctests.

### Fixed

- Serde implementations no longer do internal buffering.
- Serde implementations now reserve capacity if the size hint is available; thanks [@aatifsyed](https://github.com/aatifsyed) for your first contribution!
- A few unnecessary bounds have been loosened.

## [0.3.2] - 2025-05-24

### Added

- The hash map types now support custom hashers.
- With the new `allocator-api2` feature (enabled by default), the hash map types now support custom allocators, including on stable. See [the bumpalo-alloc example](https://github.com/oxidecomputer/iddqd/blob/940c661095cf23c97b4769c9e0fdf9b95e9a7670/crates/iddqd-extended-examples/examples/bumpalo-alloc.rs#L31).
- Added some documentation explaining iteration order.
- Added a note in the README and `lib.rs` that small copyable keys like integers are best returned as owned ones.

### Changed

- Dropped the `Ord` requirement for `Comparable` keys. (The `Hash` requirement for `Equivalent` continues to be required.)

## [0.3.1] - 2025-05-22

### Added

- Re-export `equivalent::Equivalent` and `equivalent::Comparable`.

## [0.3.0] - 2025-05-22

### Changed

- Lookups now use [`equivalent::Equivalent`] or [`equivalent::Comparable`], which are strictly more general than `Borrow`.
- `get_mut` and `remove` methods no longer require the key type; the borrow checker limitation has been worked around.

[`equivalent::Equivalent`]: https://docs.rs/equivalent/1.0.2/equivalent/trait.Equivalent.html
[`equivalent::Comparable`]: https://docs.rs/equivalent/1.0.2/equivalent/trait.Comparable.html

## [0.2.1] - 2025-05-22

### Fixed

* `MapLeaf<'a, T>`'s `Clone` and `Copy` no longer require `T` to be `Clone` or `Copy`. (`MapLeaf` is just a couple of references, so this is never necessary.)

## [0.2.0] - 2025-05-21

### Added

- `Extend` implementations.

### Changed

- Daft implementations for `BiHashMap` and `TriHashMap` changed to also allow diffing by individual keys.

## [0.1.2] - 2025-05-21

### Added

- `BiHashMap` and `TriHashMap` now have a `remove_unique` method which removes an item uniquely indexed by all keys.

### Changed

* `upcast` macros are now annotated with `#[inline]`, since they're trivial.

## [0.1.1] - 2025-05-21

### Added

- [Daft](https://docs.rs/daft) implementations with the new `daft` feature.
- `BiHashItem` implementations for reference types like `&'a T` and `Box<T>`.

## [0.1.0] - 2025-05-21

Initial release.

[0.3.16]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.16
[0.3.15]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.15
[0.3.14]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.14
[0.3.13]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.13
[0.3.12]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.12
[0.3.11]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.11
[0.3.10]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.10
[0.3.9]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.9
[0.3.8]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.8
[0.3.7]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.7
[0.3.6]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.6
[0.3.5]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.5
[0.3.4]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.4
[0.3.3]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.3
[0.3.2]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.2
[0.3.1]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.1
[0.3.0]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.0
[0.2.1]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.2.1
[0.2.0]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.2.0
[0.1.2]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.1.2
[0.1.1]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.1.1
[0.1.0]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.1.0
