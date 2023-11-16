const ERROR: &str = "[🔴]";
const WARN: &str = "[🟠]";
const INFO: &str = "[🟢]";
const LOG: &str = "[🪵]";
const CONTRACT: &str = "[📜]";

const UNICODE_PREFIXES: [&str; 5] = [ERROR, WARN, INFO, LOG, CONTRACT];

pub fn starts_with_unicode_prefix(msg: &str) -> bool {
    UNICODE_PREFIXES
        .iter()
        .any(|prefix| msg.starts_with(prefix))
}
