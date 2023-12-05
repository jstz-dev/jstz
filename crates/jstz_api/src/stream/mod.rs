use boa_engine::Context;

use self::readable::ReadableStreamApi;

pub mod readable;
mod tmp;

pub struct StreamApi;

impl jstz_core::Api for StreamApi {
    fn init(self, context: &mut Context<'_>) {
        ReadableStreamApi.init(context);
    }
}
