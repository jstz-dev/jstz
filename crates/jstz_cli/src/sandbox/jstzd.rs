use anyhow::Context;
use in_container::in_container;
use log::info;
use tokio::process::{Child, Command};
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{sleep, Duration};

use crate::{
    config::Config,
    error::{bail_user_error, Result},
    sandbox::consts::JSTZD_SERVER_BASE_URL,
};

async fn retry<'a, F>(max_attempts: u16, interval_ms: u64, f: impl Fn() -> F) -> bool
where
    F: std::future::Future<Output = bool> + Send + 'a,
{
    let duration = Duration::from_millis(interval_ms);
    for _ in 0..max_attempts {
        sleep(duration).await;
        if f().await {
            return true;
        }
    }
    false
}

async fn is_jstzd_running() -> Result<bool> {
    match reqwest::get(format!("{JSTZD_SERVER_BASE_URL}/health")).await {
        Err(e) => {
            // ignore connection error because this is what is returned when the server
            // is not running
            if !e.is_connect() {
                return Err(e.into());
            }
            Ok(false)
        }
        Ok(r) => Ok(r.status().is_success()),
    }
}

async fn shutdown_jstzd() -> Result<()> {
    reqwest::Client::new()
        .put(format!("{JSTZD_SERVER_BASE_URL}/shutdown"))
        .send()
        .await
        .context("failed to stop the sandbox")?;
    Ok(())
}

async fn run_jstzd() -> Result<Child> {
    let child = Command::new("jstzd")
        .args(["run"])
        .spawn()
        .context("failed to run jstzd")?;

    let jstzd_running = retry(70, 1000, || async {
        is_jstzd_running().await.unwrap_or_default()
    })
    .await;
    if !jstzd_running {
        bail_user_error!("jstzd did not turn healthy in time")
    }
    Ok(child)
}

pub async fn stop_sandbox(restart: bool, cfg: &mut Config) -> Result<()> {
    match is_jstzd_running().await {
        Ok(true) => {
            if !restart {
                info!("Stopping the sandbox...");
            }

            shutdown_jstzd().await?;
            cfg.jstzd_config.take();
            cfg.save()?;
            Ok(())
        }
        Ok(false) => {
            if !restart {
                bail_user_error!("The sandbox is not running!")
            } else {
                Ok(())
            }
        }
        Err(e) => bail_user_error!("Failed to check sandbox status: {e:?}"),
    }
}

pub async fn main(detach: bool, cfg: &mut Config) -> Result<()> {
    if let Ok(true) = is_jstzd_running().await {
        bail_user_error!("The sandbox is already running!");
    }

    if detach && in_container() {
        bail_user_error!("Detaching from the terminal is not supported in this environment. Please run `jstz sandbox start` without the `--detach` flag.");
    }

    let mut c = run_jstzd()
        .await
        .context("Sandbox did not launch successfully")?;
    cfg.reload().await?;
    cfg.save()?;

    if !detach {
        let mut sigterm = signal(SignalKind::terminate()).unwrap();
        let mut sigint = signal(SignalKind::interrupt()).unwrap();

        tokio::select! {
            _ = c.wait() => (),
            _ = sigterm.recv() => {
                shutdown_jstzd().await?;
            },
            _ = sigint.recv() => {
                shutdown_jstzd().await?;
            },
        };
        cfg.jstzd_config.take();
        cfg.save()?;
    }
    Ok(())
}
