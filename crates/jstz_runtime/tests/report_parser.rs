use jstz_runtime::wpt::TestHarnessReport;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("failed to parse JSON report: {0}")]
    JsonParse(String),
    #[error("failed to unescape and parse JSON report: {0}")]
    JsonUnescape(String),
}

pub fn parse_report_from_log_line(
    line: &str,
) -> Result<Option<TestHarnessReport>, ParseError> {
    const START: &str = "<REPORT_START>";
    const END: &str = "<REPORT_END>";

    // Find the last START marker, then the next END after it.
    let start_idx = match line.rfind(START) {
        Some(i) => i + START.len(),
        None => return Ok(None),
    };
    let end_idx = match line[start_idx..].find(END) {
        Some(rel) => start_idx + rel,
        None => return Ok(None),
    };

    let raw = line[start_idx..end_idx].trim();

    // First try to parse the payload as raw JSON.
    match serde_json::from_str::<TestHarnessReport>(raw) {
        Ok(report) => Ok(Some(report)),
        Err(first_err) => {
            // If the JSON was string-escaped inside the log
            // (e.g., {\"status\":null,...}), decode once as a JSON string,
            // then parse the resulting JSON.
            let mut wrapped = String::with_capacity(raw.len() + 2);
            wrapped.push('"');
            wrapped.push_str(raw);
            wrapped.push('"');

            match serde_json::from_str::<String>(&wrapped) {
                Ok(unescaped) => {
                    let report = serde_json::from_str::<TestHarnessReport>(&unescaped)
                        .map_err(|e| ParseError::JsonUnescape(e.to_string()))?;
                    Ok(Some(report))
                }
                Err(_) => Err(ParseError::JsonParse(first_err.to_string())),
            }
        }
    }
}
