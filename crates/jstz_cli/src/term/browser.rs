use std::process::{Command, Stdio};

use in_container::in_container;
use log::info;

use crate::{error::Result, term::styles};

pub fn open_browser(url: &str) -> Result<()> {
    if in_container() {
        info!(
            "Opening a link in your default browser is not supported in this environment: {}",
            styles::url(url)
        );
        return Ok(());
    }

    // TODO: Support Windows
    if cfg!(target_os = "linux") {
        let linux_cmd = format!(r#"xdg-open "{}""#, url);
        Command::new("sh")
            .arg("-c")
            .arg(&linux_cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
    } else {
        let mac_cmd = format!(r#"open "{}""#, url);
        Command::new("sh")
            .arg("-c")
            .arg(&mac_cmd)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
    };

    info!(
        "Opened a link in your default browser: {}",
        styles::url(url)
    );
    Ok(())
}
