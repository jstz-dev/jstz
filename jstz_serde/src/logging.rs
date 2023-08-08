use serde::Serialize;

use super::Address;
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Copy, Serialize)]
pub enum ConsolePrefix {
    Log,
    Error,
    Warning,
    Debug,
    Info,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Serialize)]
pub struct ConsoleMessage<'a> {
    #[serde(rename = "type")]
    pub message_type: ConsolePrefix,
    pub address: &'a Address,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub messages: &'a Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub group: &'a Vec<String>,
}

pub fn create_log_message<'a>(msg: &ConsoleMessage<'a>) -> Option<String> {
    let mut msg = serde_json::to_string(msg).ok()?;
    msg.push('\n');
    Some(msg)
}
