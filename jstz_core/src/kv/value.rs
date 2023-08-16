use std::any::{Any, TypeId};
use std::fmt::Debug;

use bincode::Options;
use derive_more::{Deref, DerefMut};
use serde::de::DeserializeOwned;

fn bincode_options() -> impl bincode::Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

pub(crate) fn serialize<T: erased_serde::Serialize + ?Sized>(value: &T) -> Vec<u8> {
    let mut writer = Vec::new();
    let mut bincode_serializer = bincode::Serializer::new(&mut writer, bincode_options());

    value
        .erased_serialize(&mut <dyn erased_serde::Serializer>::erase(
            &mut bincode_serializer,
        ))
        .expect("serialization failed");

    writer
}

pub(crate) fn deserialize<T: DeserializeOwned>(bytes: &[u8]) -> T {
    bincode::deserialize(bytes).expect("deserialization failed")
}

/// A key-value 'value' is a value that is can be dynamically
/// coerced (using `Any`) and serialized.
pub trait Value: Any + Debug + erased_serde::Serialize {}

// Since trait downcasting isn't permitted, we implement all methods
// from `dyn Any`.
impl dyn Value {
    pub fn is<T: Any>(&self) -> bool {
        let t = TypeId::of::<T>();
        let concrete = self.type_id();
        t == concrete
    }

    pub unsafe fn downcast_ref_unchecked<T: Any>(&self) -> &T {
        unsafe { &*(self as *const dyn Value as *const T) }
    }

    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        if self.is::<T>() {
            unsafe { Some(self.downcast_ref_unchecked()) }
        } else {
            None
        }
    }

    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            unsafe { Some(self.downcast_mut_unchecked()) }
        } else {
            None
        }
    }

    pub unsafe fn downcast_mut_unchecked<T: Any>(&mut self) -> &mut T {
        unsafe { &mut *(self as *mut dyn Value as *mut T) }
    }

    pub fn serialize(&self) -> Vec<u8> {
        serialize(self)
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
        if self.is::<T>() {
            Ok(unsafe { self.downcast_unchecked() })
        } else {
            Err(self)
        }
    }
}
