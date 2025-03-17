use crate::BinEncodable;
use derive_more::{Deref, DerefMut};
use std::any::Any;
use std::fmt::Debug;

/// A key-value 'value' is a value that can be dynamically
/// coerced (using `Any`) and serialized.
/// It must satisfy three requirements:
/// 1. Dynamic type coercion through `Any` trait
/// 2. Debug formatting for development and error messages
/// 3. Binary encoding/decoding through `BinEncodable` trait
///
/// Types implementing this trait can be:
/// - Stored and retrieved from the key-value store
/// - Dynamically downcasted to concrete types
/// - Cloned into new boxed values
///
/// The trait is object-safe and can be used with trait objects.
pub trait Value: Any + Debug + BinEncodable {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn clone_box(&self) -> Box<dyn Value>;
}

impl<T> Value for T
where
    T: Any + Debug + Clone + BinEncodable,
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
