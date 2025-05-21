// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::panic::AssertUnwindSafe;

pub fn catch_panic<T>(f: impl FnOnce() -> T) -> Option<T> {
    let result = std::panic::catch_unwind(AssertUnwindSafe(f));
    match result {
        Ok(value) => Some(value),
        Err(err) => {
            if let Some(err) = err.downcast_ref::<&str>() {
                eprintln!("caught panic: {}", err);
            } else {
                eprintln!("caught unknown panic");
            }
            None
        }
    }
}
