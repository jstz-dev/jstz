mod consts;
mod container;
mod jstzd;

use crate::config::Config;
use crate::error::{bail, bail_user_error};
use anyhow::Result;
use clap::Subcommand;
pub use consts::*;
use container::*;

const SANDBOX_CONTAINER_NAME: &str = "jstz-sandbox";
const SANDBOX_IMAGE: &str = "ghcr.io/jstz-dev/jstz/jstzd:0.1.1-alpha.1";

pub async fn assert_sandbox_running(sandbox_base_url: &str) -> Result<()> {
    match jstzd::is_jstzd_running(sandbox_base_url).await {
        Ok(false) => {
            bail_user_error!(
                "No sandbox is currently running. Please run {}.",
                crate::term::styles::command("jstz sandbox start")
            );
        }
        Err(e) => {
            bail!("Failed to check sandbox status: {}", e);
        }
        Ok(true) => Ok(()),
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 🎬 Starts the sandbox.
    Start {
        /// Detach the process to run in the background.
        #[clap(long, short, default_value = "false")]
        detach: bool,
    },
    /// 🛑 Stops the sandbox.
    Stop,
    /// 🔄 Restarts the sandbox.
    Restart {
        /// Detach the process to run in the background.
        #[clap(long, short, default_value = "false")]
        detach: bool,
    },
}

pub async fn start(detach: bool, use_container: bool) -> Result<()> {
    let mut cfg = Config::load().await?;

    match use_container {
        true => {
            start_container(SANDBOX_CONTAINER_NAME, SANDBOX_IMAGE, detach, &mut cfg)
                .await?
        }
        _ => jstzd::main(detach, &mut cfg).await?,
    };
    Ok(())
}

pub async fn stop(use_container: bool) -> Result<bool> {
    let mut cfg = Config::load().await?;
    match use_container {
        true => stop_container(SANDBOX_CONTAINER_NAME, &mut cfg).await,
        _ => {
            jstzd::stop_sandbox(false, &mut cfg).await?;
            Ok(true)
        }
    }
}

pub async fn restart(detach: bool, use_container: bool) -> Result<()> {
    if !stop(use_container).await? {
        return Ok(());
    }
    start(detach, use_container).await
}

pub async fn exec(use_container: bool, command: Command) -> Result<()> {
    match command {
        Command::Start { detach } => start(detach, use_container).await,
        Command::Stop => {
            stop(use_container).await?;
            Ok(())
        }
        Command::Restart { detach } => restart(detach, use_container).await,
    }
}

#[cfg(test)]
mod tests {
    use super::assert_sandbox_running;

    #[tokio::test]
    async fn assert_sandbox_running_ok() {
        let mut server = mockito::Server::new_async().await;
        server.mock("GET", "/health").create();
        assert!(assert_sandbox_running(&server.url()).await.is_ok());
    }

    #[tokio::test]
    async fn assert_sandbox_running_not_running() {
        assert!(assert_sandbox_running("http://dummy/")
            .await
            .is_err_and(|e| e.to_string().contains("No sandbox is currently running.")));
    }

    #[tokio::test]
    async fn assert_sandbox_running_other_errors() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/health")
            .with_status(500)
            .with_body("foo")
            .create();
        assert_eq!(
            assert_sandbox_running("bad_url")
                .await
                .unwrap_err()
                .to_string(),
            "Failed to check sandbox status: builder error: relative URL without a base"
        );
    }
}
