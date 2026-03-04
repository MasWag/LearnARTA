// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Location (state) identifiers for ARTA.

/// A location identifier.
///
/// Locations are identified by a unique string name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocationId(String);

impl LocationId {
    /// Create a new location identifier.
    ///
    /// # Arguments
    ///
    /// * `name` — any value that can be converted into a `String`; used as the unique name of the location.
    pub fn new(name: impl Into<String>) -> Self {
        LocationId(name.into())
    }

    /// The string name of this location.
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for LocationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for LocationId {
    fn from(s: &str) -> Self {
        LocationId(s.to_string())
    }
}

impl From<String> for LocationId {
    fn from(s: String) -> Self {
        LocationId(s)
    }
}
