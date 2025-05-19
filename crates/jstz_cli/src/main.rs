use clap::Parser;
use jstz_cli::{error, exec, term, Command};
use log::debug;
#[tokio::main]
async fn main() {
    term::init_logger();

    let command = Command::parse();

    debug!("Command: {:?}", command);

    if let Err(err) = exec(command).await {
        error::print(&err);
        std::process::exit(1);
    }
}
