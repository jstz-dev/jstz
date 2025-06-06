use crate::runtime::v2::fetch::http::{Body, Response};
use deno_core::{AsyncResult, BufView, Resource};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use std::future::Future;
use std::pin::Pin;
use std::{cell::RefCell, rc::Rc};
use url::Url;

pub struct FetchRequestResource {
    pub future: Pin<Box<dyn Future<Output = Response>>>,
    pub url: Url,
    #[allow(dead_code)]
    pub from: SmartFunctionHash,
}

impl Resource for FetchRequestResource {}

pub struct FetchResponseResource {
    pub body: RefCell<Option<Body>>,
}

impl Resource for FetchResponseResource {
    fn read(self: Rc<Self>, _limit: usize) -> AsyncResult<BufView> {
        Box::pin(async move {
            if let Some(body) = self.body.borrow_mut().take() {
                return Ok(match body {
                    Body::Buffer(body) => BufView::from(body),
                    Body::Vector(body) => BufView::from(body),
                });
            }
            Ok(BufView::empty())
        })
    }
}
