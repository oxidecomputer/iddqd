// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::fmt;

/// Debug impl for a static string without quotes.
pub(crate) struct StrDisplayAsDebug(pub(crate) &'static str);

impl fmt::Debug for StrDisplayAsDebug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use the Display formatter to write the string without quotes.
        fmt::Display::fmt(&self.0, f)
    }
}
