use boa_engine::Context;

use self::{
    global::GlobalApi, text_decoder::TextDecoderApi, text_encoder::TextEncoderApi,
};

pub mod global;
pub mod text_decoder;
pub mod text_encoder;

pub struct EncodingApi;

impl jstz_core::Api for EncodingApi {
    fn init(self, context: &mut Context<'_>) {
        TextEncoderApi.init(context);
        TextDecoderApi.init(context);
        GlobalApi.init(context);
    }
}
