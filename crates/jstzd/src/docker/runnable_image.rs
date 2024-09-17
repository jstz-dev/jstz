use crate::docker::Image;
use std::{collections::BTreeMap, net::IpAddr};

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
}
