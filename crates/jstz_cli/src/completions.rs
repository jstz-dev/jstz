use std::io;

use clap::CommandFactory;
use clap_complete::Shell;

use crate::{error::Result, Command};

pub fn exec(shell: Shell) -> Result<()> {
    let cmd = &mut Command::command();
    clap_complete::generate(shell, cmd, "jstz", &mut io::stdout());
    Ok(())
}
