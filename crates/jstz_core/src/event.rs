use futures::Stream;
use serde::{de::DeserializeOwned, Serialize};

/// Jstz Events
pub trait Event: PartialEq + Serialize + DeserializeOwned {
    fn tag() -> &'static str;
}

/// Stream of Jstz Events
pub trait EventStream: Stream<Item = Result<Self::T, Self::E>> + Unpin {
    type T: Event;
    type E;
}

impl<T, E, S> EventStream for S
where
    T: Event,
    S: Stream<Item = Result<T, E>> + Unpin,
{
    type T = T;
    type E = E;
}
