#![allow(unused)]
mod console;
mod runtime;

pub(crate) use runtime::JstzHostRuntime;
pub use runtime::JstzRuntime;

#[cfg(test)]
mod test {
    use std::ops::{Deref, DerefMut};
    use tezos_smart_rollup_mock::MockHost as RollupMockHost;

    pub struct MockHostRuntime {
        inner: RollupMockHost,
        #[allow(clippy::box_collection)]
        sink: Box<Vec<u8>>,
    }

    impl MockHostRuntime {
        pub fn sink(&self) -> &[u8] {
            &self.sink
        }

        pub fn init() -> Self {
            let sink: Box<Vec<u8>> = Box::default();
            let rollup_host = RollupMockHost::default();
            let mut host = Self {
                inner: rollup_host,
                sink,
            };
            host.inner.set_debug_handler(unsafe {
                std::mem::transmute::<&mut std::vec::Vec<u8>, &'static mut Vec<u8>>(
                    host.sink.as_mut(),
                )
            });

            host
        }
    }

    impl Deref for MockHostRuntime {
        type Target = RollupMockHost;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl DerefMut for MockHostRuntime {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }
}
