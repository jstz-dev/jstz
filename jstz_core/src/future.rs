use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

/// A simple blocking scheduler for Rust futures

struct Signal;

impl Wake for Signal {
    fn wake(self: Arc<Self>) {}
}

pub fn block_on<F: Future>(mut fut: F) -> F::Output {
    let mut fut = unsafe { std::pin::Pin::new_unchecked(&mut fut) };

    let waker = Waker::from(Arc::new(Signal));
    let mut context = Context::from_waker(&waker);

    loop {
        match fut.as_mut().poll(&mut context) {
            Poll::Pending => (),
            Poll::Ready(item) => break item,
        }
    }
}
