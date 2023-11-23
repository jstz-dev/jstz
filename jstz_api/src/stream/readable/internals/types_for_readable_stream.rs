// https://streams.spec.whatwg.org/#rs-class-definition

use crate::js_aware::JsOptional;

use super::super::{
    readable_stream::ReadableStream,
    readable_stream_byob_reader::ReadableStreamBYOBReader,
    readable_stream_default_reader::ReadableStreamDefaultReader,
};

use derive_more::*;
use derive_more::{IsVariant, Unwrap};
use enum_as_inner::EnumAsInner;

#[derive(From, TryInto, EnumAsInner)]
pub enum ReadableStreamReader {
    DefaultReader(ReadableStreamDefaultReader),
    BYOBReader(ReadableStreamBYOBReader),
}
/*
macro_rules! impl_MaybeAsRef_and_MaybeAsMut_for_ReadableStreamReader {
    ($t:ty, $c:ident) => {
        impl MaybeAsRef<$t> for ReadableStreamReader {
            fn maybe_as_ref(&self) -> Option<&$t> {
                match self {
                    ReadableStreamReader::$c(ref reader) => Some(reader),
                    _ => None,
                }
            }
        }

        impl MaybeAsMut<$t> for ReadableStreamReader {
            fn maybe_as_mut(&mut self) -> Option<&mut $t> {
                match self {
                    ReadableStreamReader::$c(ref mut reader) => Some(reader),
                    _ => None,
                }
            }
        }
    };
}

impl_MaybeAsRef_and_MaybeAsMut_for_ReadableStreamReader!(
    ReadableStreamDefaultReader,
    DefaultReader
);

impl_MaybeAsRef_and_MaybeAsMut_for_ReadableStreamReader!(
    ReadableStreamBYOBReader,
    BYOBReader
);
 */

#[derive(From, TryInto, EnumAsInner)]
pub enum ReadableStreamReaderMode {
    BYOB,
}

pub struct ReadableStreamGetReaderOptions {
    pub mode: JsOptional<ReadableStreamReaderMode>,
}

impl Default for ReadableStreamGetReaderOptions {
    fn default() -> Self {
        Self {
            mode: Default::default(),
        }
    }
}

pub struct ReadableStreamIteratorOptions {
    // TODO optional fields
    prevent_cancel: bool,
}

impl Default for ReadableStreamIteratorOptions {
    fn default() -> Self {
        ReadableStreamIteratorOptions {
            prevent_cancel: false,
        }
    }
}

pub struct ReadableWritablePair {
    readable: ReadableStream,
    //writable: WritableStream, // TODO
}

pub struct StreamPipeOptions {
    prevent_close: bool,
    prevent_abort: bool,
    prevent_cancel: bool,
    //signal: AbortSignal, // TODO add signal, and replace the Default implementation by something else
}

impl Default for StreamPipeOptions {
    fn default() -> Self {
        StreamPipeOptions {
            prevent_close: false,
            prevent_abort: false,
            prevent_cancel: false,
        }
    }
}

// https://streams.spec.whatwg.org/#rs-internal-slots

#[derive(PartialEq, IsVariant, Unwrap, Clone)]
pub enum ReadableStreamState {
    Readable,
    Closed,
    Errored,
}
