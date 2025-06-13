use std::fmt::{self, Display};

use jstz_core::{
    host::{HostRuntime, JsHostRuntime},
    runtime,
};
use jstz_crypto::smart_function_hash::SmartFunctionHash;
use serde::{Deserialize, Serialize};

pub const REQUEST_START_PREFIX: &str = "[JSTZ:SMART_FUNCTION:REQUEST_START] ";
pub const REQUEST_END_PREFIX: &str = "[JSTZ:SMART_FUNCTION:REQUEST_END] ";

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum RequestEvent {
    Start {
        address: SmartFunctionHash,
        request_id: String,
    },
    End {
        address: SmartFunctionHash,
        request_id: String,
        // TODO: Add more fields
    },
}

impl Display for RequestEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            &serde_json::to_string(self).expect("Failed to convert RequestLog to string"),
        )
    }
}

impl RequestEvent {
    pub fn try_from_string(json: &str) -> Option<Self> {
        serde_json::from_str(json).ok()
    }
}

pub fn log_request_start(address: SmartFunctionHash, request_id: String) {
    runtime::with_js_hrt(|hrt| {
        log_request_start_with_host(hrt, address, request_id);
    });
}

pub fn log_request_start_with_host(
    hrt: &mut JsHostRuntime<'static>,
    address: SmartFunctionHash,
    request_id: String,
) {
    let request_log = RequestEvent::Start {
        address,
        request_id,
    }
    .to_string();

    hrt.write_debug(&(REQUEST_START_PREFIX.to_string() + &request_log + "\n"));
}

pub fn log_request_end(address: SmartFunctionHash, request_id: String) {
    runtime::with_js_hrt(|hrt| {
        log_request_end_with_host(hrt, address, request_id);
    });
}

pub fn log_request_end_with_host(
    hrt: &mut JsHostRuntime<'static>,
    address: SmartFunctionHash,
    request_id: String,
) {
    let request_log = RequestEvent::End {
        address,
        request_id,
    }
    .to_string();

    hrt.write_debug(&(REQUEST_END_PREFIX.to_string() + &request_log + "\n"));
}

#[cfg(test)]
mod tests {
    use jstz_core::{host::JsHostRuntime, kv::Transaction};
    use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
    use tezos_smart_rollup_mock::MockHost;

    use crate::tests::DebugLogSink;

    #[test]
    fn log_request_start() {
        let sink = DebugLogSink::new();
        let buf = sink.content();
        let mut host = MockHost::default();
        host.set_debug_handler(sink);
        jstz_core::runtime::enter_js_host_context(
            &mut JsHostRuntime::new(&mut host),
            &mut Transaction::default(),
            || {
                super::log_request_start(
                    SmartFunctionHash::from_base58(
                        "KT1D5U6oBmtvYmjBtjzR5yPbrzxw8fa2kCn9",
                    )
                    .unwrap(),
                    "start_request".to_string(),
                )
            },
        );
        assert_eq!(String::from_utf8(buf.lock().unwrap().to_vec()).unwrap(), "[JSTZ:SMART_FUNCTION:REQUEST_START] {\"type\":\"Start\",\"address\":\"KT1D5U6oBmtvYmjBtjzR5yPbrzxw8fa2kCn9\",\"request_id\":\"start_request\"}\n");
    }

    #[test]
    fn log_request_start_with_host() {
        let sink = DebugLogSink::new();
        let buf = sink.content();
        let mut host = MockHost::default();
        host.set_debug_handler(sink);
        super::log_request_start_with_host(
            &mut JsHostRuntime::new(&mut host),
            SmartFunctionHash::from_base58("KT1D5U6oBmtvYmjBtjzR5yPbrzxw8fa2kCn9")
                .unwrap(),
            "foobar".to_string(),
        );
        assert_eq!(String::from_utf8(buf.lock().unwrap().to_vec()).unwrap(), "[JSTZ:SMART_FUNCTION:REQUEST_START] {\"type\":\"Start\",\"address\":\"KT1D5U6oBmtvYmjBtjzR5yPbrzxw8fa2kCn9\",\"request_id\":\"foobar\"}\n");
    }

    #[test]
    fn log_request_end() {
        let sink = DebugLogSink::new();
        let buf = sink.content();
        let mut host = MockHost::default();
        host.set_debug_handler(sink);
        jstz_core::runtime::enter_js_host_context(
            &mut JsHostRuntime::new(&mut host),
            &mut Transaction::default(),
            || {
                super::log_request_end(
                    SmartFunctionHash::from_base58(
                        "KT1D5U6oBmtvYmjBtjzR5yPbrzxw8fa2kCn9",
                    )
                    .unwrap(),
                    "end_request".to_string(),
                )
            },
        );
        assert_eq!(String::from_utf8(buf.lock().unwrap().to_vec()).unwrap(), "[JSTZ:SMART_FUNCTION:REQUEST_END] {\"type\":\"End\",\"address\":\"KT1D5U6oBmtvYmjBtjzR5yPbrzxw8fa2kCn9\",\"request_id\":\"end_request\"}\n");
    }

    #[test]
    fn log_request_end_with_host() {
        let sink = DebugLogSink::new();
        let buf = sink.content();
        let mut host = MockHost::default();
        host.set_debug_handler(sink);
        super::log_request_end_with_host(
            &mut JsHostRuntime::new(&mut host),
            SmartFunctionHash::from_base58("KT1D5U6oBmtvYmjBtjzR5yPbrzxw8fa2kCn9")
                .unwrap(),
            "foobar".to_string(),
        );
        assert_eq!(String::from_utf8(buf.lock().unwrap().to_vec()).unwrap(), "[JSTZ:SMART_FUNCTION:REQUEST_END] {\"type\":\"End\",\"address\":\"KT1D5U6oBmtvYmjBtjzR5yPbrzxw8fa2kCn9\",\"request_id\":\"foobar\"}\n");
    }
}
