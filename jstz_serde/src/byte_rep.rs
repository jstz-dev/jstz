use itertools::{Either, Itertools};
use postcard::{from_bytes, to_stdvec};
use serde::{Deserialize, Serialize};
use std::iter::Iterator;
use std::num::ParseIntError;
use std::str::Utf8Error;
use std::{marker::PhantomData, str::FromStr};

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct ByteRep<T> {
    bytes: Vec<u8>,
    phantom: PhantomData<T>,
}

impl<T> ByteRep<T> {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            phantom: PhantomData::default(),
        }
    }
    pub fn bytes(&self) -> &Vec<u8> {
        &self.bytes
    }
}
impl<T: Serialize> ByteRep<T> {
    pub fn from_t(source: &T) -> Self {
        source.into()
    }
}

// rust orphan rules prevent implementation of TryInto
impl<T> ByteRep<T>
where
    T: for<'a> Deserialize<'a>,
{
    pub fn into_t(&self) -> Result<T, postcard::Error> {
        from_bytes(self.bytes())
    }
}

impl<T: Serialize> From<&T> for ByteRep<T> {
    fn from(source: &T) -> Self {
        let bytes = to_stdvec(source).unwrap();
        Self::new(bytes)
    }
}

impl<T> ToString for ByteRep<T> {
    fn to_string(&self) -> String {
        self.bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .join("")
    }
}

impl<T> FromStr for ByteRep<T> {
    type Err = Either<Utf8Error, ParseIntError>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn map_byte(byte: &[u8]) -> Result<u8, Either<Utf8Error, ParseIntError>> {
            let as_str = std::str::from_utf8(byte).map_err(Either::Left)?;
            u8::from_str_radix(as_str, 16).map_err(Either::Right)
        }
        let bytes: Result<Vec<u8>, _> = s.as_bytes().chunks(2).map(map_byte).collect();
        Ok(Self::new(bytes?))
    }
}
