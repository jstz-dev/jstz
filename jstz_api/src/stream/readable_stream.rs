use boa_engine::Context;
pub struct ReadableStreamApi;

impl jstz_core::Api for ReadableStreamApi {
    fn init(self, context: &mut Context<'_>) {
        //register_global_class::<RequestClass>(context)
        //    .expect("The `Request` class shouldn't exist yet")
    }
}
