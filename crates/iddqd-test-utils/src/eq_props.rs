// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::fmt;

/// Assert equality properties.
///
/// The PartialEq algorithms under test are not obviously symmetric or
/// reflexive, so we must ensure in our tests that they are.
#[allow(clippy::eq_op)]
pub fn assert_eq_props<T: Eq + fmt::Debug>(a: T, b: T) {
    assert_eq!(a, a, "a == a");
    assert_eq!(b, b, "b == b");
    assert_eq!(a, b, "a == b");
    assert_eq!(b, a, "b == a");
}

/// Assert inequality properties.
///
/// The PartialEq algorithms in this crate are not obviously symmetric or
/// reflexive, so we must ensure in our tests that they are.
#[allow(clippy::eq_op)]
pub fn assert_ne_props<T: Eq + fmt::Debug>(a: T, b: T) {
    // Also check reflexivity while we're here.
    assert_eq!(a, a, "a == a");
    assert_eq!(b, b, "b == b");
    assert_ne!(a, b, "a != b");
    assert_ne!(b, a, "b != a");
}
