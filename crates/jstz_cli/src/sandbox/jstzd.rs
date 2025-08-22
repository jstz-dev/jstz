use std::path::PathBuf;

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

pub(crate) async fn is_jstzd_running(jstzd_server_base_url: &str) -> Result<bool> {
    match reqwest::get(format!("{jstzd_server_base_url}/health")).await {
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

async fn shutdown_jstzd(jstzd_server_base_url: &str) -> Result<()> {
    reqwest::Client::new()
        .put(format!("{jstzd_server_base_url}/shutdown"))
        .send()
        .await
        .context("failed to stop the sandbox")?;
    Ok(())
}

async fn run_jstzd(
    jstzd_server_base_url: &str,
    jstzd_config: Option<PathBuf>,
) -> Result<Child> {
    let additional_args = match jstzd_config {
        Some(p) => vec![p.canonicalize()?],
        None => vec![],
    };
    let mut args = vec!["run"];
    args.append(
        &mut additional_args
            .iter()
            .map(|x| x.to_str().expect("Invalid jstzd config path"))
            .collect(),
    );
    let mut child = Command::new("jstzd")
        .args(args.as_slice())
        .spawn()
        .context("failed to run jstzd")?;

    let jstzd_running = retry(70, 1000, || async {
        is_jstzd_running(jstzd_server_base_url)
            .await
            .unwrap_or_default()
    });

    let succesful = tokio::select! {
    child_result = child.wait() => {
        match child_result {
            Err(e) => {
                bail_user_error!("{}", e);
            },
            Ok(exit_status) =>
                if exit_status.success() {
                    // If we exited successfully, then the child process was
                    // correctly spawned, so wait for for `jstzd_running`
                    std::future::pending::<bool>().await
                }
                else {
                    false
                }
            }

    }
    retry_result = jstzd_running => {
            retry_result
        }
    };

    if !succesful {
        bail_user_error!("jstzd did not turn healthy in time")
    }
    Ok(child)
}

async fn _stop_sandbox(
    jstzd_server_base_url: &str,
    restart: bool,
    cfg: &mut Config,
) -> Result<()> {
    match is_jstzd_running(jstzd_server_base_url).await {
        Ok(true) => {
            if !restart {
                info!("Stopping the sandbox...");
            }

            shutdown_jstzd(jstzd_server_base_url).await?;
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

pub async fn stop_sandbox(restart: bool, cfg: &mut Config) -> Result<()> {
    _stop_sandbox(JSTZD_SERVER_BASE_URL, restart, cfg).await
}

pub async fn main(
    detach: bool,
    cfg: &mut Config,
    jstzd_config: Option<PathBuf>,
) -> Result<()> {
    let jstzd_server_base_url = JSTZD_SERVER_BASE_URL;
    if let Ok(true) = is_jstzd_running(jstzd_server_base_url).await {
        bail_user_error!("The sandbox is already running!");
    }

    if detach && in_container() {
        bail_user_error!("Detaching from the terminal is not supported in this environment. Please run `jstz sandbox start` without the `--detach` flag.");
    }

    let mut c = run_jstzd(jstzd_server_base_url, jstzd_config)
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
                shutdown_jstzd(jstzd_server_base_url).await?;
            },
            _ = sigint.recv() => {
                shutdown_jstzd(jstzd_server_base_url).await?;
            },
        };
        cfg.jstzd_config.take();
        cfg.save()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use octez::unused_port;

    use crate::{
        config::{
            Config, JstzNodeConfig, JstzdConfig, OctezClientConfig, OctezNodeConfig,
        },
        sandbox::jstzd::{_stop_sandbox, is_jstzd_running},
    };

    #[tokio::test]
    async fn is_jstzd_running_err() {
        assert_eq!(
            is_jstzd_running("").await.unwrap_err().to_string(),
            "builder error: relative URL without a base"
        );
    }

    #[tokio::test]
    async fn is_jstzd_running_server_not_listening() {
        let port = unused_port();
        assert!(!is_jstzd_running(&format!("http://127.0.0.1/{port}"))
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn is_jstzd_running_unhealthy() {
        let mut server = mockito::Server::new_async().await;
        server.mock("GET", "/health").with_status(500).create();
        assert!(!is_jstzd_running(&server.url()).await.unwrap());
    }

    #[tokio::test]
    async fn is_jstzd_running_ok() {
        let mut server = mockito::Server::new_async().await;
        server.mock("GET", "/health").create();
        assert!(is_jstzd_running(&server.url()).await.unwrap());
    }

    #[tokio::test]
    async fn stop_sandbox_err() {
        let mut cfg = Config::default();
        assert!(_stop_sandbox("", false, &mut cfg)
            .await
            .is_err_and(|e| e.to_string().starts_with("Failed to check sandbox status")));
    }

    #[tokio::test]
    async fn stop_sandbox_not_running() {
        let mut server = mockito::Server::new_async().await;
        server.mock("GET", "/health").with_status(500).create();

        let mut cfg = Config::default();
        assert_eq!(
            _stop_sandbox(&server.url(), false, &mut cfg)
                .await
                .unwrap_err()
                .to_string(),
            "The sandbox is not running!"
        );

        assert!(_stop_sandbox(&server.url(), true, &mut cfg).await.is_ok());
    }

    #[tokio::test]
    async fn stop_sandbox_ok() {
        let mut server = mockito::Server::new_async().await;
        server.mock("GET", "/health").create();
        server.mock("PUT", "/shutdown").create();

        let mut cfg = Config::default();
        cfg.jstzd_config.replace(JstzdConfig {
            octez_client: OctezClientConfig {
                base_dir: String::new(),
            },
            octez_node: OctezNodeConfig {
                rpc_endpoint: String::new(),
            },
            jstz_node: JstzNodeConfig {
                endpoint: String::new(),
            },
        });
        assert!(_stop_sandbox(&server.url(), false, &mut cfg).await.is_ok());
        assert!(cfg.jstzd_config.is_none());
    }
}
