use serde::{Deserialize, Serialize};
use std::{convert::Infallible, fmt::Display};

// we might decide to store an address as something other than a string
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Serialize, Deserialize)]
pub struct Address(String);

impl Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl Into<String> for Address {
    fn into(self) -> String {
        self.0
    }
}
impl TryFrom<String> for Address {
    type Error = Infallible;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(Self(value))
    }
}
impl TryFrom<&str> for Address {
    type Error = Infallible;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.to_string().try_into()
    }
}
