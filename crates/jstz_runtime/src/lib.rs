pub mod runtime;

pub use runtime::JstzRuntime;

#[cfg(test)]
mod test_utils {

    use tezos_smart_rollup_mock::MockHost;

    #[allow(unused)]
    #[allow(clippy::box_collection)]
    pub fn init_mock_host() -> (Box<Vec<u8>>, MockHost) {
        let mut sink: Box<Vec<u8>> = Box::default();
        let mut host = MockHost::default();
        host.set_debug_handler(unsafe {
            std::mem::transmute::<&mut std::vec::Vec<u8>, &'static mut Vec<u8>>(
                sink.as_mut(),
            )
        });

        (sink, host)
    }
}
