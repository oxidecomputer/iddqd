/// Re-export `Global` if allocator-api2 isn't enabled; it's not public but is
/// used within tests.
pub use crate::support::alloc::Global;
use alloc::string::String;
use core::fmt;

/// Bails out of a validation function with a [`TableValidationError`].
///
/// This panics under Soteria and returns an error otherwise. (Soteria can't
/// handle `format!` too well at the moment.)
macro_rules! table_validation_fail {
    ($lit:literal $(, $arg:expr)* $(,)?) => {{
        #[cfg(soteria)]
        ::soteria::panic($lit);
        #[cfg(not(soteria))]
        return ::core::result::Result::Err(
            $crate::internal::TableValidationError::new(::alloc::format!(
                $lit $(, $arg)*
            )),
        );
    }};
}

/// Bails out of a validation function with a general [`ValidationError`].
///
/// This panics under Soteria and returns an error otherwise. (Soteria can't
/// handle `format!` too well at the moment.)
macro_rules! general_validation_fail {
    ($lit:literal $(, $arg:expr)* $(,)?) => {{
        #[cfg(soteria)]
        ::soteria::panic($lit);
        #[cfg(not(soteria))]
        return ::core::result::Result::Err(
            $crate::internal::ValidationError::General(::alloc::format!(
                $lit $(, $arg)*
            )),
        );
    }};
}

pub(crate) use general_validation_fail;
pub(crate) use table_validation_fail;

/// For validation, indicate whether we expect integer tables to be compact
/// (have all values in the range 0..table.len()).
///
/// Maps are expected to be compact if no remove operations were performed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidateCompact {
    Compact,
    NonCompact,
}

/// For validation, indicates whether chaos testing is in effect.
///
/// If it is, then we fall back to linear searches for table lookups.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidateChaos {
    Yes,
    No,
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
                write!(f, "validation error in table {name}: {error}")
            }
            Self::General(msg) => msg.fmt(f),
        }
    }
}

impl core::error::Error for ValidationError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
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

impl core::error::Error for TableValidationError {}
