use boa_engine::{Context, JsError, JsResult, JsValue};
use boa_gc::{custom_trace, Finalize, Trace};

use crate::idl;

// https://streams.spec.whatwg.org/#default-reader-class-definition

pub struct ReadableStreamReadResult {
    any: JsValue,
    done: bool,
}

impl Finalize for ReadableStreamReadResult {
    fn finalize(&self) {
        // TODO check
        self.any.finalize();
    }
}

unsafe impl Trace for ReadableStreamReadResult {
    custom_trace!(this, {
        // TODO check
        mark(&this.any);
    });
}

// https://streams.spec.whatwg.org/#default-reader-internal-slots

pub trait ReadRequestTrait {
    // TODO check return types
    fn chunk_steps(self: &Self, chunk: &idl::Chunk, context: &mut Context<'_>) -> ();
    fn close_steps(self: &Self, context: &mut Context<'_>) -> ();
    fn error_steps(self: &Self, context: &mut Context<'_>) -> ();
}

pub type ReadRequest = Box<dyn ReadRequestTrait>;

impl Clone for ReadRequest {
    fn clone(&self) -> Self {
        todo!()
    }
}

// https://streams.spec.whatwg.org/#byob-reader-internal-slots

pub trait ReadIntoRequestTrait {
    // TODO check return types
    fn chunk_steps(self: &Self, chunk: &idl::Chunk, context: &mut Context<'_>) -> ();
    fn close_steps(
        self: &Self,
        chunk: &idl::ChunkOrUndefined,
        context: &mut Context<'_>,
    ) -> ();
    fn error_steps(self: &Self, error: &JsError, context: &mut Context<'_>) -> ();
}

pub type ReadIntoRequest = Box<dyn ReadIntoRequestTrait>;

impl Clone for ReadIntoRequest {
    fn clone(&self) -> Self {
        todo!()
    }
}
