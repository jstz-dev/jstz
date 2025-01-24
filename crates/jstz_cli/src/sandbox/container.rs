use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result};
use bollard::{
    container::{
        AttachContainerOptions, AttachContainerResults, Config as ContainerConfig,
        CreateContainerOptions, ListContainersOptions, RemoveContainerOptions,
    },
    image::{CreateImageOptions, ListImagesOptions},
    secret::{HostConfig, Mount, MountTypeEnum, PortBinding},
    Docker,
};
use futures::TryStreamExt;
use futures_util::StreamExt;
use signal_hook::consts::{SIGINT, SIGTERM};

pub use super::consts::*;
use log::info;
use tempfile::{NamedTempFile, TempDir};
use tokio::{fs, io::AsyncWriteExt};

use crate::config::{Config, SandboxConfig};

const JSTZD_OCTEZ_CLIENT_DIR_PATH: &str = "/jstzd/octez-client-dir";
const JSTZD_CONFIG_PATH: &str = "/jstzd/config.json";

/// Starts a container that runs jstzd.
pub(crate) async fn start_container(
    container_name: &str,
    image: &str,
    detach: bool,
    cfg: &mut Config,
) -> Result<()> {
    let client =
        Docker::connect_with_defaults().context("Failed to connect to docker")?;
    if container_exists(&client, container_name).await? {
        return Err(anyhow::anyhow!("Sandbox is already running"));
    }

    pull_image_if_not_found(&client, image).await?;

    let (tmp_dir_path, config_file_path) = create_config_file_and_client_dir().await?;
    let mounts = Some(HashMap::from_iter([
        (
            tmp_dir_path.to_string_lossy().to_string(),
            JSTZD_OCTEZ_CLIENT_DIR_PATH.to_owned(),
        ),
        (
            config_file_path.to_string_lossy().to_string(),
            JSTZD_CONFIG_PATH.to_owned(),
        ),
    ]));
    create_container(
        &client,
        container_name,
        image,
        mounts,
        Some(vec![SANDBOX_OCTEZ_NODE_RPC_PORT, SANDBOX_JSTZ_NODE_PORT]),
        Some(vec!["run".to_owned(), JSTZD_CONFIG_PATH.to_owned()]),
    )
    .await
    .context("Failed to create the sandbox container")?;
    client
        .start_container::<&str>(container_name, None)
        .await
        .context("Failed to start the sandbox container")?;

    // update config so that the following CLI commands can call the sandbox
    cfg.sandbox = Some(SandboxConfig {
        octez_client_dir: tmp_dir_path,
        octez_node_dir: PathBuf::new(),
        octez_rollup_node_dir: PathBuf::new(),
        pid: 0,
        container: true,
    });
    cfg.save()?;

    if !detach {
        // Docker::start_container returns immediately after the container is up and there is
        // no way to make it behave like `docker run -it`. Therefore, if users wish to attach
        // to the session and see the output, we need to manually attach to the running container.
        attach_container(&client, container_name, cfg.clone())
            .await
            .context("Failed to attach to the sandbox")?;
    }

    Ok(())
}

pub(crate) async fn stop_container(
    container_name: &str,
    cfg: &mut Config,
) -> Result<bool> {
    let client =
        Docker::connect_with_defaults().context("Failed to connect to docker")?;
    if container_exists(&client, container_name).await? {
        client
            .remove_container(
                container_name,
                Some(RemoveContainerOptions {
                    v: true,
                    force: true,
                    link: false,
                }),
            )
            .await?;
        cfg.sandbox.take();
        cfg.save()?;
        info!("Sandbox stopped");
        Ok(true)
    } else {
        info!("Sandbox is not running");
        Ok(false)
    }
}

async fn container_exists(client: &Docker, target: &str) -> Result<bool> {
    let containers = client
        .list_containers(Some(ListContainersOptions::<&str> {
            all: true,
            filters: HashMap::from_iter([("name", vec![target])]),
            ..Default::default()
        }))
        .await
        .context("Failed to list containers")?;
    for container in containers {
        if let Some(names) = container.names {
            for name in names {
                // for some reason, the returned names may have a "/" prefix
                if name.strip_prefix("/").unwrap_or(&name) == target {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

async fn pull_image_if_not_found(client: &Docker, image: &str) -> Result<()> {
    let images = client
        .list_images(Some(ListImagesOptions::<&str> {
            all: true,
            filters: HashMap::from_iter([("reference", vec![image])]),
            ..Default::default()
        }))
        .await?;

    for summary in images {
        for tag in summary.repo_tags {
            if tag == image {
                return Ok(());
            }
        }
    }

    info!("Sandbox image '{image}' does not exist locally. Trying to pull it from the remote repository...");
    let _ = client
        .create_image(
            Some(CreateImageOptions {
                from_image: image,
                ..Default::default()
            }),
            None,
            None,
        )
        .try_collect::<Vec<_>>()
        .await?;
    Ok(())
}

/// Creates a basic jstzd config file and a temporary directory that serves as the base
/// directory of the octez client used in a sandbox.
// These two things are mounted into the container. The temporary directory can then be
// shared by both the octez client inside and outside the container. With the basic
// config, jstzd inside the container then tells octez client to use the mounted base
// directory and assigns certain port numbers to octez node and jstz node so that they
// can be exposed to the host environment.
async fn create_config_file_and_client_dir() -> Result<(PathBuf, PathBuf)> {
    let tmp_dir_path = TempDir::new()
        .context("Failed to create temporary directory for octez client")?
        .into_path();
    let content = serde_json::to_string(&serde_json::json!({
        "octez_client": {
            "octez_node_endpoint": format!("http://0.0.0.0:{SANDBOX_OCTEZ_NODE_RPC_PORT}"),
            "base_dir": JSTZD_OCTEZ_CLIENT_DIR_PATH,
        },
        "octez_node": {
            "rpc_endpoint": format!("0.0.0.0:{SANDBOX_OCTEZ_NODE_RPC_PORT}")
        },
    })).context("Failed to serialise sandbox config")?;
    let config_file_path = NamedTempFile::new()
        .context("Failed to create a file as the config file")?
        .into_temp_path()
        .to_path_buf();
    fs::File::create(&config_file_path)
        .await
        .context("Failed to create config file")?
        .write_all(content.as_bytes())
        .await
        .context("Failed to write config file")?;
    Ok((tmp_dir_path, config_file_path))
}

async fn create_container(
    client: &Docker,
    container_name: &str,
    image: &str,
    mounts: Option<HashMap<String, String>>,
    ports: Option<Vec<u16>>,
    cmd: Option<Vec<String>>,
) -> Result<()> {
    client
        .create_container(
            new_create_container_options(container_name),
            new_create_container_config(image, mounts, ports, cmd),
        )
        .await?;
    Ok(())
}

fn new_create_container_options(
    container_name: &str,
) -> Option<CreateContainerOptions<&str>> {
    Some(CreateContainerOptions::<&str> {
        name: container_name,
        ..Default::default()
    })
}

fn new_create_container_config(
    image: &str,
    mounts: Option<HashMap<String, String>>,
    ports: Option<Vec<u16>>,
    cmd: Option<Vec<String>>,
) -> ContainerConfig<String> {
    ContainerConfig {
        image: Some(image.to_owned()),
        host_config: Some(HostConfig {
            mounts: create_mounts(mounts),
            port_bindings: create_port_bindings(ports.as_ref()),
            auto_remove: Some(true),
            network_mode: Some("bridge".to_string()),
            ..Default::default()
        }),
        attach_stdin: Some(true),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        open_stdin: Some(true),
        exposed_ports: create_exposed_ports(ports.as_ref()),
        cmd,
        ..Default::default()
    }
}

fn create_port_bindings(
    ports: Option<&Vec<u16>>,
) -> Option<HashMap<String, Option<Vec<PortBinding>>>> {
    ports.map(|v| {
        HashMap::from_iter(v.iter().map(|p| {
            (
                format!("{p}/tcp").to_string(),
                Some(vec![PortBinding {
                    host_ip: None,
                    host_port: Some(p.to_string()),
                }]),
            )
        }))
    })
}

fn create_exposed_ports(
    ports: Option<&Vec<u16>>,
) -> Option<HashMap<String, HashMap<(), ()>>> {
    ports.map(|v| {
        HashMap::from_iter(
            v.iter()
                .map(|p| (format!("{p}/tcp").to_string(), HashMap::new())),
        )
    })
}

fn create_mounts(mapping: Option<HashMap<String, String>>) -> Option<Vec<Mount>> {
    mapping.map(|v| {
        v.iter()
            .map(|(source, target)| Mount {
                source: Some(source.to_owned()),
                target: Some(target.to_owned()),
                typ: Some(MountTypeEnum::BIND),
                ..Default::default()
            })
            .collect::<Vec<_>>()
    })
}

/// Attach to a running container. This function streams the output of the container to stdout
/// of the current process and terminates the container when SIGINT/SIGTERM comes in.
async fn attach_container(
    client: &Docker,
    container_name: &str,
    mut cfg: Config,
) -> Result<()> {
    let options = Some(AttachContainerOptions::<String> {
        stdin: Some(true),
        stdout: Some(true),
        stderr: Some(true),
        stream: Some(true),
        logs: Some(true),
        ..Default::default()
    });

    let AttachContainerResults { mut output, .. } =
        client.attach_container(container_name, options).await?;
    let mut signals = signal_hook::iterator::Signals::new([SIGINT, SIGTERM])?;
    let signal_handle = signals.handle();

    let name = container_name.to_owned();
    tokio::spawn(async move {
        if signals.forever().next().is_some() {
            stop_container(&name, &mut cfg).await.unwrap();
        }
    });

    let mut stdout = tokio::io::stdout();
    let mut container_status_channel =
        client.wait_container::<&str>(container_name, None);

    loop {
        tokio::select! {
            // We need this status channel to determine when it is okay to close
            // everything and leave because the output stream does not close even
            // after the container terminates.
            // https://github.com/fussybeaver/bollard/issues/251
            // This handles premature container exits, e.g. jstzd shutting down due
            // to internal bugs, container being killed externally, etc.
            _ = container_status_channel.next() => {
                // close the handle so that signals.forever() can stop running and therefore
                // let the process exit
                signal_handle.close();
                break;
            },
            Some(Ok(output)) = output.next() => {
                // pipe docker attach output into stdout
                stdout.write_all(output.into_bytes().as_ref()).await?;
                stdout.flush().await?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::sandbox::SANDBOX_OCTEZ_NODE_RPC_PORT;
    use bollard::{
        container::{Config as ContainerConfig, CreateContainerOptions},
        secret::{HostConfig, Mount, MountTypeEnum, PortBinding},
    };
    use serde_json::Value;
    use std::collections::HashMap;

    #[test]
    fn create_exposed_ports() {
        assert_eq!(super::create_exposed_ports(None), None);
        assert_eq!(
            super::create_exposed_ports(Some(&vec![1234, 5678])),
            Some(HashMap::from_iter([
                ("1234/tcp".to_owned(), HashMap::new()),
                ("5678/tcp".to_owned(), HashMap::new())
            ]))
        );
    }

    #[test]
    fn create_port_bindings() {
        assert_eq!(super::create_port_bindings(None), None);
        assert_eq!(
            super::create_port_bindings(Some(&vec![1234, 5678])),
            Some(HashMap::from_iter([
                (
                    "1234/tcp".to_owned(),
                    Some(vec![PortBinding {
                        host_ip: None,
                        host_port: Some("1234".to_owned()),
                    }])
                ),
                (
                    "5678/tcp".to_owned(),
                    Some(vec![PortBinding {
                        host_ip: None,
                        host_port: Some("5678".to_owned()),
                    }])
                )
            ]))
        );
    }

    #[test]
    fn create_mounts() {
        assert_eq!(super::create_mounts(None), None);
        assert_eq!(super::create_mounts(Some(HashMap::new())), Some(Vec::new()));
        assert_eq!(
            super::create_mounts(Some(HashMap::from_iter([(
                "/foo".to_owned(),
                "/bar".to_owned()
            )]))),
            Some(vec![Mount {
                source: Some("/foo".to_owned()),
                target: Some("/bar".to_owned()),
                typ: Some(MountTypeEnum::BIND),
                ..Default::default()
            }])
        );
    }

    #[test]
    fn new_create_container_config() {
        let cmd = Some(vec!["cmd".to_owned()]);
        let mounts = Some(HashMap::from_iter([("/foo".to_owned(), "/bar".to_owned())]));
        assert_eq!(
            super::new_create_container_config(
                "test-image",
                mounts.clone(),
                Some(vec![1234]),
                cmd.clone()
            ),
            ContainerConfig {
                image: Some("test-image".to_owned()),
                host_config: Some(HostConfig {
                    mounts: Some(vec![Mount {
                        source: Some("/foo".to_owned()),
                        target: Some("/bar".to_owned()),
                        typ: Some(MountTypeEnum::BIND),
                        ..Default::default()
                    }]),
                    port_bindings: Some(HashMap::from_iter([(
                        "1234/tcp".to_owned(),
                        Some(vec![PortBinding {
                            host_ip: None,
                            host_port: Some("1234".to_owned()),
                        }])
                    )])),
                    network_mode: Some("bridge".to_owned()),
                    auto_remove: Some(true),
                    ..Default::default()
                }),
                attach_stdin: Some(true),
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                open_stdin: Some(true),
                exposed_ports: Some(HashMap::from_iter([(
                    "1234/tcp".to_owned(),
                    HashMap::new()
                ),])),
                cmd,
                ..Default::default()
            }
        );
    }

    #[test]
    fn new_create_container_options() {
        assert_eq!(
            super::new_create_container_options("foo"),
            Some(CreateContainerOptions::<&str> {
                name: "foo",
                ..Default::default()
            })
        );
    }

    #[tokio::test]
    async fn create_config_file_and_client_dir() {
        let (_, cfg_path) = super::create_config_file_and_client_dir().await.unwrap();

        let value: Value =
            serde_json::from_str(&tokio::fs::read_to_string(cfg_path).await.unwrap())
                .unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "octez_client": {
                    "octez_node_endpoint": format!("http://0.0.0.0:{SANDBOX_OCTEZ_NODE_RPC_PORT}"),
                    "base_dir": super::JSTZD_OCTEZ_CLIENT_DIR_PATH,
                },
                "octez_node": {
                    "rpc_endpoint": format!("0.0.0.0:{SANDBOX_OCTEZ_NODE_RPC_PORT}")
                },
            })
        );
    }
}
