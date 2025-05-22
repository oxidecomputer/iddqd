# Changelog

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

[0.2.0]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.2.0
[0.1.2]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.1.2
[0.1.1]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.1.1
[0.1.0]: https://github.com/oxidecomputer/iddqd/releases/iddqd-0.1.0
