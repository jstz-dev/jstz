use boa_engine::Context;

pub mod writable_stream;

pub struct WritableStreamApi;

impl jstz_core::Api for WritableStreamApi {
    fn init(self, context: &mut Context<'_>) {
        // TODO
    }
}
