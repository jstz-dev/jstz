#![cfg(feature = "wpt-in-riscv")]
use jstz_runtime::wpt::TestHarnessReport;
use ron::de::from_str as ron_from_str;
use serde::Deserialize;
use std::borrow::Cow;
use thiserror::Error;

#[derive(Deserialize)]
pub struct LogLine<'a> {
    pub message: Cow<'a, str>,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("log line contained a report, but it couldn't be parsed as RON: {0}")]
    Ron(String),
}

/// Try to get a TestHarnessReport out of any line of the giventext
pub fn parse_report_from_log_line(
    line: &str,
) -> Result<Option<TestHarnessReport>, ParseError> {
    if let Some(raw_dbg) = slice_report_debug(line) {
        return parse_report_debug_text(raw_dbg).map(Some);
    }

    if let Ok(env) = serde_json::from_str::<LogLine>(line) {
        if let Some(raw_dbg) = slice_report_debug(&env.message) {
            return parse_report_debug_text(raw_dbg).map(Some);
        }
    }

    Ok(None)
}

/// Parse the debug text "TestHarnessReport { ... }" into a struct
fn parse_report_debug_text(raw_dbg: &str) -> Result<TestHarnessReport, ParseError> {
    match try_ron_from_debug(raw_dbg) {
        Ok(rep) => Ok(rep),
        Err(_) => {
            if let Some(unescaped) = try_unescape_as_json_string(raw_dbg) {
                try_ron_from_debug(&unescaped).map_err(|e| ParseError::Ron(e))
            } else {
                Err(ParseError::Ron(
                    "RON parse failed and JSON-unescape also failed".into(),
                ))
            }
        }
    }
}

/// Convert to RON and attempt to parse
fn try_ron_from_debug(debug_text: &str) -> Result<TestHarnessReport, String> {
    let ron_text = braces_to_parens_preserving_strings(debug_text);
    ron_from_str::<TestHarnessReport>(&ron_text).map_err(|e| e.to_string())
}

fn try_unescape_as_json_string(raw: &str) -> Option<String> {
    if !(raw.contains("\\\"")
        || raw.contains("\\n")
        || raw.contains("\\t")
        || raw.contains("\\r")
        || raw.contains("\\\\"))
    {
        return None;
    }
    let wrapped = format!("\"{}\"", raw);
    serde_json::from_str::<String>(&wrapped).ok()
}

/// Extracts "TestHarnessReport { ... }" from any text.
fn slice_report_debug(s: &str) -> Option<&str> {
    let start = s.find("TestHarnessReport")?;
    let after = &s[start..];

    if let Some(end_sentinel) = after.find("Internal message:") {
        let candidate = &after[..end_sentinel];
        if candidate.find('{').is_some() {
            return Some(candidate.trim());
        }
    }

    let open_rel = after.find('{')?;
    let open_idx = start + open_rel;

    let mut depth: i32 = 0;
    let mut in_str = false;
    let mut prev_bs = false;
    let mut end_idx = None;

    for (i, ch) in s[open_idx..].char_indices() {
        if in_str {
            match ch {
                '\\' => prev_bs = !prev_bs,
                '"' if !prev_bs => {
                    in_str = false;
                    prev_bs = false;
                }
                _ => prev_bs = false,
            }
            continue;
        }

        match ch {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = Some(open_idx + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }

    end_idx.map(|end| s[start..end].trim())
}

/// Replace `{`/`}` with `(`/`)` outside of quoted strings.
fn braces_to_parens_preserving_strings(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_str = false;
    let mut prev_bs = false;

    for ch in input.chars() {
        if in_str {
            out.push(ch);
            match ch {
                '\\' => prev_bs = !prev_bs,
                '"' if !prev_bs => {
                    in_str = false;
                    prev_bs = false;
                }
                _ => prev_bs = false,
            }
        } else {
            match ch {
                '"' => {
                    in_str = true;
                    out.push(ch);
                }
                '{' => out.push('('),
                '}' => out.push(')'),
                _ => out.push(ch),
            }
        }
    }

    out
}
