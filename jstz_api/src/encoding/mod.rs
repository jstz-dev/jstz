mod text_decoder;
mod text_encoder;

pub struct EncodingApi;

impl jstz_core::Api for EncodingApi {
    fn init(self, context: &mut boa_engine::Context<'_>) {
        text_decoder::TextDecoderApi.init(context);
        text_encoder::TextEncoderApi.init(context);
    }
}
