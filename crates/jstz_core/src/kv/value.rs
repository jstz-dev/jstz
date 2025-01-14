use std::any::Any;
use std::fmt::Debug;

use derive_more::{Deref, DerefMut};
use erased_serde::serialize_trait_object;
use serde::de::DeserializeOwned;

use crate::{Error, Result, BINCODE_CONFIGURATION};

pub fn serialize<T: ?Sized + serde::Serialize>(value: &T) -> Result<Vec<u8>> {
    let result =
        bincode::serde::encode_to_vec(value, BINCODE_CONFIGURATION).map_err(|err| {
            Error::SerializationError {
                description: format!("{err}"),
            }
        })?;

    Ok(result)
}

pub(crate) fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let (result, _) = bincode::serde::decode_from_slice(bytes, BINCODE_CONFIGURATION)
        .map_err(|err| Error::SerializationError {
            description: format!("{err}"),
        })?;

    Ok(result)
}

/// A key-value 'value' is a value that is can be dynamically
/// coerced (using `Any`) and serialized.
pub trait Value: Any + Debug + erased_serde::Serialize {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn clone_box(&self) -> Box<dyn Value>;
}

serialize_trait_object!(Value);

impl<T> Value for T
where
    T: Any + Debug + Clone + erased_serde::Serialize,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn Value> {
        Box::new(self.clone())
    }
}

#[derive(Debug, Deref, DerefMut)]
pub(crate) struct BoxedValue(Box<dyn Value>);

impl Clone for BoxedValue {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

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
