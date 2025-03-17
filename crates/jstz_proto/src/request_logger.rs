use std::fmt::{self, Display};

use jstz_core::{host::HostRuntime, runtime};
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
    let request_log = RequestEvent::Start {
        address,
        request_id,
    }
    .to_string();

    runtime::with_js_hrt(|hrt| {
        hrt.write_debug(&(REQUEST_START_PREFIX.to_string() + &request_log + "\n"));
    });
}

pub fn log_request_end(address: SmartFunctionHash, request_id: String) {
    let request_log = RequestEvent::End {
        address,
        request_id,
    }
    .to_string();

    runtime::with_js_hrt(|hrt| {
        hrt.write_debug(&(REQUEST_END_PREFIX.to_string() + &request_log + "\n"));
    });
}
