use anyhow::{anyhow, Result};
pub use jstz_proto::runtime::v2::oracle::request::OracleRequest;
use once_cell::sync::Lazy;
use regex::Regex;

pub use jstz_proto::runtime::v2::oracle::request::OracleRequest;

// [ORACLE]{"id":1, ... }
pub(crate) static ORACLE_LINE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\[ORACLE\]\s*(?P<json>\{.*\})\s*$"#).expect("hard-coded regex")
});

pub fn request_event_from_log_line(line: &str) -> Result<OracleRequest> {
    jstz_proto::event::decode_line::<OracleRequest>(line).map_err(|e| anyhow!(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jstz_crypto::public_key_hash::PublicKeyHash;
    use jstz_proto::runtime::v2::fetch::http::Request as HttpReq;
    use std::str::FromStr;
    use url::Url;

    use jstz_crypto::public_key_hash::PublicKeyHash;

    fn make_json(id: u64) -> String {
        let http_req = HttpReq {
            method: "GET".into(),
            url: Url::parse("http://example.com").unwrap(),
            headers: vec![],
            body: None,
        };
        let oracle = OracleRequest {
            id,
            caller: PublicKeyHash::from_str("tz1cD5CuvAALcxgypqBXcBQEA8dkLJivoFjU")
                .unwrap(), // 20-byte placeholder
            gas_limit: 0,
            timeout: 0,
            request: http_req,
        };
        serde_json::to_string(&oracle).unwrap()
    }

    #[test]
    fn parses_valid_line() {
        let line = format!("[ORACLE]{}", make_json(42));
        let ev = request_event_from_log_line(&line).unwrap();
        assert_eq!(ev.id, 42);
        assert_eq!(ev.request.url, Url::parse("http://example.com").unwrap());
    }

    #[test]
    fn rejects_missing_tag() {
        let line = make_json(1);
        let err = request_event_from_log_line(&line).unwrap_err();
        assert!(
            err.to_string()
                .contains("Nom decode failed: kind 'Tag' on input '"),
            "{err}"
        );
    }

    #[test]
    fn rejects_garbage_json() {
        let line = "[ORACLE]{ this is not json }";
        let err = request_event_from_log_line(line).unwrap_err();
        assert!(err.to_string().contains("key must be a string"), "{err}");
    }

    #[test]
    fn rejects_unterminated_json() {
        let line = "[ORACLE]{\"id\":1";
        let err = request_event_from_log_line(line).unwrap_err();
        assert!(err.to_string().contains("EOF while parsing"), "{err}");
    }
}
