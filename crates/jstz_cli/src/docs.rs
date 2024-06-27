use log::warn;

use crate::{error::Result, term::open_browser};

const DOCS_URL: &str = "https://trilitech.github.io/jstz/";

pub fn exec() -> Result<()> {
    if open_browser(DOCS_URL).is_err() {
        warn!("Failed to open a link in your default browser.");
    }

    Ok(())
}
