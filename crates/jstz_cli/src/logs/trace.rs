use anyhow::Result;
use std::process::{Command, Stdio};

use crate::Config;

const ERROR: &str = "🔴";
const WARN: &str = "🟠";
const INFO: &str = "🟢";
const LOG: &str = "🪵";
const CONTRACT: &str = "📜";

pub fn exec(
    log: bool,
    info: bool,
    warn: bool,
    error: bool,
    contract: bool,
    custom: Vec<String>,
    cfg: &Config,
) -> Result<()> {
    let logs_dir = cfg.jstz_path.join("logs");
    let log_path = logs_dir.join("kernel.log");

    let mut grep_for = Vec::new();
    if log {
        grep_for.push(LOG.to_string());
    }
    if info {
        grep_for.push(INFO.to_string());
    }
    if warn {
        grep_for.push(WARN.to_string());
    }
    if error {
        grep_for.push(ERROR.to_string());
    }
    if contract {
        grep_for.push(CONTRACT.to_string());
    }
    for s in &custom {
        grep_for.push(s.clone());
    }

    if grep_for.is_empty() {
        grep_for.extend(
            [LOG, INFO, WARN, ERROR, CONTRACT]
                .iter()
                .map(|&s| s.to_string()),
        );
    }

    let grep_pattern = grep_for.join("\\|");

    let tail = Command::new("tail")
        .arg("-f")
        .arg(log_path.clone())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start tail command");

    if let Some(tail_stdout) = tail.stdout {
        Command::new("grep")
            .arg(grep_pattern)
            .stdin(tail_stdout)
            .spawn()
            .expect("Failed to start grep command")
            .wait()
            .expect("Failed to wait for grep command");
    }

    Ok(())
}
