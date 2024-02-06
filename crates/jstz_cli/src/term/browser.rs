use std::process::{Command, Stdio};

use log::info;

use crate::{error::Result, term::styles};

pub fn open_browser(url: &str) -> Result<()> {
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
