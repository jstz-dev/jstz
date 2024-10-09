use boa_engine::Context;

use self::{header::HeadersApi, request::RequestApi, response::ResponseApi};

pub mod body;
pub mod header;
pub mod request;
pub mod response;

pub struct HttpApi;

impl jstz_core::Api for HttpApi {
    fn init(self, context: &mut Context) {
        HeadersApi.init(context);
        RequestApi.init(context);
        ResponseApi.init(context);
    }
}
