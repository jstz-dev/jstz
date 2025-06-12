use anyhow::Result;
use anyhow::{anyhow, Context};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

pub(crate) static ORACLE_LINE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\[ORACLE\]\s+(?P<id>\d+)\s*$"#).unwrap()); // [ORACLE] <id>

// TODO: unify with oracle
pub(crate) type RequestId = u64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OracleRequestEvent {
    pub id: RequestId,
    pub request: OracleRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OracleRequest {
    pub request: u64, // TODO: complete and unify with oracle
}

fn parse_request_id(line: &str) -> Result<RequestId> {
    let caps = ORACLE_LINE_REGEX
        .captures(line)
        .ok_or_else(|| anyhow!("line does not match oracle pattern"))?;
    let id_str = caps
        .name("id")
        .context("regex missing <id> capture")?
        .as_str();
    id_str.parse::<RequestId>().context("id is not a u64")
}

// TODO: implement properly getting oracle request from the rollup storage
fn load_request_event(id: RequestId) -> Result<OracleRequestEvent> {
    Ok(OracleRequestEvent {
        id,
        request: OracleRequest { request: 0 },
    })
}

pub(crate) fn request_event_from_log_line(line: &str) -> Result<OracleRequestEvent> {
    let id = parse_request_id(line)?;
    let request = load_request_event(id)?;
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_id(line: &str) -> RequestId {
        parse_request_id(line).expect("should parse")
    }

    fn err_id(line: &str) -> String {
        parse_request_id(line).unwrap_err().to_string()
    }

    #[test]
    fn parses_valid_line() {
        assert_eq!(ok_id("[ORACLE] 0"), 0);
        assert_eq!(ok_id("[ORACLE] 42"), 42);
        assert_eq!(ok_id("[ORACLE] 999999"), 999_999);
    }

    #[test]
    fn rejects_non_numeric_id() {
        let msg = err_id("[ORACLE] abc");
        assert!(msg.contains("line does not match oracle pattern"), "{msg}");
    }

    #[test]
    fn rejects_missing_tag() {
        let msg = err_id("[FOO] 1");
        assert!(msg.contains("does not match oracle pattern"), "{msg}");
    }

    #[test]
    fn rejects_trailing_garbage() {
        let msg = err_id("[ORACLE] 7 extra");
        assert!(msg.contains("does not match oracle pattern"), "{msg}");
    }

    #[test]
    fn builds_event_with_stub_request() {
        let ev = request_event_from_log_line("[ORACLE] 123").unwrap();
        assert_eq!(ev.id, 123);
        assert_eq!(ev.request.request, 0);
    }
}
