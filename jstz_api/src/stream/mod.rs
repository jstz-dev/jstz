use boa_engine::Context;

use self::{readable::ReadableStreamApi, writable::WritableStreamApi};

pub mod readable;
pub mod strategy;
pub mod writable;

pub struct StreamApi;

impl jstz_core::Api for StreamApi {
    fn init(self, context: &mut Context<'_>) {
        ReadableStreamApi.init(context);
        WritableStreamApi.init(context);
    }
}
