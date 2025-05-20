// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::fmt;

/// For validation, indicate whether we expect integer tables to be compact
/// (have all values in the range 0..table.len()).
///
/// Maps are expected to be compact if no remove operations were performed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidateCompact {
    Compact,
    NonCompact,
}

#[derive(Debug)]
pub enum ValidationError {
    Table { name: &'static str, error: TableValidationError },
    General(String),
}

impl ValidationError {
    pub(crate) fn general(msg: impl Into<String>) -> Self {
        ValidationError::General(msg.into())
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Table { name, error } => {
                write!(f, "validation error in table {}: {}", name, error)
            }
            Self::General(msg) => msg.fmt(f),
        }
    }
}

impl std::error::Error for ValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ValidationError::Table { error, .. } => Some(error),
            ValidationError::General(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct TableValidationError(String);

impl TableValidationError {
    pub(crate) fn new(msg: impl Into<String>) -> Self {
        TableValidationError(msg.into())
    }
}

impl fmt::Display for TableValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for TableValidationError {}
