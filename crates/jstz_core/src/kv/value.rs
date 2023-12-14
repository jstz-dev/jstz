use std::any::Any;
use std::fmt::Debug;

use bincode::Options;
use derive_more::{Deref, DerefMut};
use serde::de::DeserializeOwned;

use crate::{Error, Result};

fn bincode_options() -> impl bincode::Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

pub fn serialize<T: erased_serde::Serialize + ?Sized>(value: &T) -> Result<Vec<u8>> {
    let mut writer = Vec::new();
    let mut bincode_serializer = bincode::Serializer::new(&mut writer, bincode_options());

    value
        .erased_serialize(&mut <dyn erased_serde::Serializer>::erase(
            &mut bincode_serializer,
        ))
        .map_err(|err| Error::SerializationError {
            description: format!("{err}"),
        })?;

    Ok(writer)
}

pub(crate) fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    bincode::deserialize(bytes).map_err(|err| Error::SerializationError {
        description: format!("{err}"),
    })
}

/// A key-value 'value' is a value that is can be dynamically
/// coerced (using `Any`) and serialized.
pub trait Value: Any + Debug + erased_serde::Serialize {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T> Value for T
where
    T: Any + Debug + erased_serde::Serialize,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Debug, Deref, DerefMut)]
pub(crate) struct BoxedValue(Box<dyn Value>);

impl BoxedValue {
    pub fn new(value: impl Value) -> Self {
        BoxedValue(Box::new(value))
    }

    pub unsafe fn downcast_unchecked<T: Any>(self) -> Box<T> {
        let raw: *mut dyn Value = Box::into_raw(self.0);
        Box::from_raw(raw as *mut T)
    }

    pub fn downcast<T>(self) -> std::result::Result<Box<T>, Self>
    where
        T: Any,
    {
        if self.as_any().is::<T>() {
            Ok(unsafe { self.downcast_unchecked() })
        } else {
            Err(self)
        }
    }
}
