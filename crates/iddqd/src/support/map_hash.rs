// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::hash::{BuildHasher, Hash, RandomState};

/// Packages up a state and a hash for later validation.
#[derive(Debug)]
pub(crate) struct MapHash {
    pub(super) state: RandomState,
    pub(super) hash: u64,
}

impl MapHash {
    pub(crate) fn is_same_hash<K: Hash>(&self, key: K) -> bool {
        self.hash == self.state.hash_one(key)
    }
}
