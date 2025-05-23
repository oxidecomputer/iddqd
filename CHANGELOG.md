# Changelog

## [0.3.2] - 2025-05-23

### Added

- Added a note in the README and `lib.rs` that small copyable keys like integers are best returned as owned ones.

### Changed

Dropped the `Ord` requirement for `Comparable` keys. (The `Hash` requirement for `Equivalent` has to remain.)

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

[0.3.2]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.2
[0.3.1]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.1
[0.3.0]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.3.0
[0.2.1]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.2.1
[0.2.0]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.2.0
[0.1.2]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.1.2
[0.1.1]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.1.1
[0.1.0]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.1.0
