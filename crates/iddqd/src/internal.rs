// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/// For validation, indicate whether we expect integer tables to be compact
/// (have all values in the range 0..table.len()).
///
/// Maps are expected to be compact if no remove operations were performed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidateCompact {
    Compact,
    NonCompact,
}
