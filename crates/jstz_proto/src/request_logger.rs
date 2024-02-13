use std::fmt::Display;

use jstz_core::{host::HostRuntime, runtime};
use serde::{Deserialize, Serialize};

use crate::context::account::Address;

pub const REQUEST_START_PREFIX: &str = "[JSTZ:SMART_FUNCTION:REQUEST_START] ";
pub const REQUEST_END_PREFIX: &str = "[JSTZ:SMART_FUNCTION:REQUEST_END] ";

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum RequestEvent {
    Start {
        address: Address,
        request_id: String,
    },
    End {
        address: Address,
        request_id: String,
        // TODO: Add more fields
    },
}

impl Display for RequestEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

pub fn log_request_start(address: Address, request_id: String) {
    let request_log = RequestEvent::Start {
        address,
        request_id,
    }
    .to_string();

    runtime::with_js_hrt(|hrt| {
        hrt.write_debug(&(REQUEST_START_PREFIX.to_string() + &request_log + "\n"));
    });
}

pub fn log_request_end(address: Address, request_id: String) {
    let request_log = RequestEvent::End {
        address,
        request_id,
    }
    .to_string();

    runtime::with_js_hrt(|hrt| {
        hrt.write_debug(&(REQUEST_END_PREFIX.to_string() + &request_log + "\n"));
    });
}
