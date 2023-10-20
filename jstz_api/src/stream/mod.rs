use boa_engine::Context;

use self::readable_stream::ReadableStreamApi;

pub mod readable_stream;

pub struct StreamApi;

impl jstz_core::Api for StreamApi {
    fn init(self, context: &mut Context<'_>) {
        ReadableStreamApi.init(context);
    }
}
