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

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[tokio::test]
    async fn test_read_vector() {
        let data = vec![1, 2, 3, 4, 5];
        let resource = Rc::new(FetchResponseResource {
            body: RefCell::new(Some(Body::Vector(data.clone()))),
        });
        let buf_view = resource.read(1024).await.unwrap();
        assert_eq!(buf_view.as_ref(), data.as_slice());
    }
}
