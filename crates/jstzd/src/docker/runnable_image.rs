use bollard::{
    container::{Config, CreateContainerOptions},
    secret::{ContainerCreateResponse, HostConfig, PortBinding, PortMap},
    Docker,
};

use crate::docker::Image;
use anyhow::Result;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::IpAddr,
    sync::Arc,
};

use super::Container;

#[derive(Debug, PartialEq, Clone)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, PartialEq, Clone)]
pub enum MountType {
    Bind,
    Volume,
    Tmpfs,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Mount {
    pub access_mode: AccessMode,
    pub mount_type: MountType,
    pub source: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Host {
    Addr(IpAddr),
    HostGateway,
}

impl ToString for Host {
    fn to_string(&self) -> String {
        match self {
            Host::Addr(ip) => ip.to_string(),
            Host::HostGateway => "host-gateway".to_string(),
        }
    }
}

type HostPort = u16;
type ContainerPort = u16;
pub struct RunnableImage<I: Image> {
    pub image: I,
    pub container_name: String,
    pub overridden_cmd: Option<Vec<String>>,
    pub network: Option<String>, //TODO: https://linear.app/tezos/issue/JSTZ-111/support-for-adding-network-to-a-container
    pub env_vars: BTreeMap<String, String>,
    pub hosts: BTreeMap<String, Host>,
    pub mounts: Vec<Mount>, //TODO: https://linear.app/tezos/issue/JSTZ-110/implement-mounts-for-runnable-image
    pub ports: BTreeMap<HostPort, ContainerPort>,
}

impl<I: Image> RunnableImage<I> {
    pub fn new(image: I, container_name: impl Into<String>) -> Self {
        Self {
            image,
            container_name: container_name.into(),
            overridden_cmd: None,
            network: None,
            env_vars: BTreeMap::new(),
            hosts: BTreeMap::new(),
            mounts: Vec::new(),
            ports: BTreeMap::new(),
        }
    }

    pub fn with_overridden_cmd(mut self, cmd: Vec<String>) -> Self {
        self.overridden_cmd = Some(cmd);
        self
    }

    pub fn with_network(mut self, network: impl Into<String>) -> Self {
        self.network = Some(network.into());
        self
    }

    pub fn with_env_var(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    pub fn with_host(mut self, hostname: impl Into<String>, host: Host) -> Self {
        self.hosts.insert(hostname.into(), host);
        self
    }

    pub fn with_mount(mut self, mount: Mount) -> Self {
        self.mounts.push(mount);
        self
    }

    pub fn with_port(
        mut self,
        host_port: HostPort,
        container_port: ContainerPort,
    ) -> Self {
        self.ports.insert(host_port, container_port);
        self
    }

    pub async fn create_container(
        self,
        client: Arc<Docker>,
    ) -> anyhow::Result<Container> {
        self.all_container_ports_are_exposed()?;
        let config = Config::<String> {
            image: Some(self.image.image_uri().to_string()),
            host_config: Some(self.host_config()),
            entrypoint: self.entrypoint(),
            cmd: self.overridden_cmd.clone(),
            env: self.env(),
            ..Config::default()
        };
        let options = Some(CreateContainerOptions {
            name: self.container_name.clone(),
            platform: None,
        });
        match client.create_container(options, config).await {
            Ok(ContainerCreateResponse { id, .. }) => Ok(Container::new(client, id)),
            Err(err) => {
                let err_msg = err.to_string();
                if err_msg.contains("404") {
                    return Err(anyhow::anyhow!(
                        "Image: {} not found, please make sure the image is pulled",
                        self.image.image_uri()
                    ));
                }
                return Err(err.into());
            }
        }
    }

    fn env(&self) -> Option<Vec<String>> {
        match self.env_vars.is_empty() {
            true => None,
            false => Some(
                self.env_vars
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>(),
            ),
        }
    }

    fn entrypoint(&self) -> Option<Vec<String>> {
        self.image.entrypoint().map(|e| {
            e.split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        })
    }

    fn host_config(&self) -> HostConfig {
        let port_bindings: PortMap = self.ports.iter().fold(
            HashMap::new(),
            |mut acc, (host_port, container_port)| {
                let container_port = container_port.to_string();
                let binding = PortBinding {
                    host_ip: None,
                    host_port: Some(host_port.to_string()),
                };
                acc.entry(container_port)
                    .and_modify(|bindings| {
                        if let Some(vec) = bindings {
                            vec.push(binding.clone());
                        }
                    })
                    .or_insert(Some(vec![binding]));
                acc
            },
        );
        let extra_hosts = self
            .hosts
            .iter()
            .map(|(host, ip)| format!("{}:{}", host, ip.to_string()))
            .collect::<Vec<_>>();
        let mut config = HostConfig::default();
        if !port_bindings.is_empty() {
            config.port_bindings = Some(port_bindings);
        }
        if !extra_hosts.is_empty() {
            config.extra_hosts = Some(extra_hosts);
        }
        config
    }
    // Check if the ports use the exposed container ports from the image if any
    fn all_container_ports_are_exposed(&self) -> Result<()> {
        let exposed_ports: HashSet<&u16> = self.image.exposed_ports().iter().collect();
        if exposed_ports.is_empty() {
            return Ok(());
        }
        let all_ports_are_exposed =
            self.ports.values().all(|port| exposed_ports.contains(port));
        if !all_ports_are_exposed {
            return Err(anyhow::anyhow!("Invalid ports"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::docker::GenericImage;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn builds_runnable_image() {
        let image = GenericImage::new("busybox");
        let cmd = vec!["echo", "hello"]
            .into_iter()
            .map(String::from)
            .collect();
        let mount = Mount {
            access_mode: AccessMode::ReadWrite,
            mount_type: MountType::Bind,
            source: Some("/source".to_string()),
            target: Some("/target".to_string()),
        };
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let runnable_image = RunnableImage::new(image, "busybox")
            .with_overridden_cmd(cmd)
            .with_network("some_network")
            .with_env_var("KEY", "VALUE")
            .with_host("hostname", Host::Addr(ip))
            .with_mount(mount.clone())
            .with_port(5000, 3000);

        assert_eq!(runnable_image.container_name, "busybox");
        assert_eq!(
            runnable_image.overridden_cmd,
            Some(vec!["echo".to_string(), "hello".to_string()])
        );
        assert_eq!(runnable_image.network, Some("some_network".to_string()));
        assert_eq!(
            runnable_image.env_vars,
            vec![("KEY".to_string(), "VALUE".to_string())]
                .into_iter()
                .collect()
        );
        assert_eq!(
            runnable_image.hosts,
            vec![("hostname".to_string(), Host::Addr(ip))]
                .into_iter()
                .collect()
        );
        assert_eq!(runnable_image.mounts, vec![mount]);
        assert_eq!(
            runnable_image.ports,
            vec![(5000, 3000)].into_iter().collect()
        );
    }

    #[test]
    fn parses_env() {
        let image = GenericImage::new("busybox");
        let runnable_image =
            RunnableImage::new(image, "busybox_test").with_env_var("KEY", "VALUE");
        let env: Option<Vec<String>> = runnable_image.env();
        assert_eq!(env, Some(vec!["KEY=VALUE".to_string()]));
    }

    #[test]
    fn parses_entrypoint() {
        let image = GenericImage::new("busybox").with_entrypoint("echo hello");
        let runnable_image = RunnableImage::new(image, "busybox_test");
        let entrypoint: Option<Vec<String>> = runnable_image.entrypoint();
        assert_eq!(
            entrypoint,
            Some(vec!["echo".to_string(), "hello".to_string()])
        );
    }

    #[test]
    fn parses_host_config() {
        let image = GenericImage::new("busybox");
        let runnable_image = RunnableImage::new(image, "busybox_test")
            .with_host("hostname", Host::HostGateway)
            .with_port(4000, 3000)
            .with_port(5000, 3000);
        let host_config = runnable_image.host_config();
        assert_eq!(
            host_config.extra_hosts,
            Some(vec!["hostname:host-gateway".to_string()])
        );

        let port_bindings = host_config.port_bindings.expect("port bindings not found");
        assert_eq!(
            port_bindings.get("3000").unwrap(),
            &Some(vec![
                PortBinding {
                    host_ip: None,
                    host_port: Some("4000".to_string())
                },
                PortBinding {
                    host_ip: None,
                    host_port: Some("5000".to_string())
                }
            ])
        );
    }

    #[test]
    fn throws_invalid_ports() {
        let image = GenericImage::new("busybox").with_exposed_ports(&[3000]);
        let runnable_image =
            RunnableImage::new(image, "busybox_test").with_port(3001, 3001);
        let result = runnable_image.all_container_ports_are_exposed();
        assert!(result.is_err());
    }

    #[test]
    fn validates_ports() {
        let image = GenericImage::new("busybox").with_exposed_ports(&[3000, 4000]);
        let runnable_image = RunnableImage::new(image, "busybox_test")
            .with_port(3000, 3000)
            .with_port(3000, 4000);
        let result = runnable_image.all_container_ports_are_exposed();
        assert!(result.is_ok());
    }
}
