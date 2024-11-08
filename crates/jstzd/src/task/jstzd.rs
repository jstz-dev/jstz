use super::{octez_node::OctezNode, Task};
use anyhow::Result;
use async_dropper_simple::AsyncDrop;
use async_trait::async_trait;
use axum::Router;
use octez::r#async::node_config::OctezNodeConfig;
use tokio::{net::TcpListener, task::JoinHandle};

struct Jstzd {
    octez_node: OctezNode,
}

#[derive(Clone)]
pub struct JstzdConfig {
    octez_node_config: OctezNodeConfig,
}

impl JstzdConfig {
    pub fn new(octez_node_config: OctezNodeConfig) -> Self {
        Self { octez_node_config }
    }
}

#[async_trait]
impl Task for Jstzd {
    type Config = JstzdConfig;

    async fn spawn(config: Self::Config) -> Result<Self> {
        Ok(Self {
            octez_node: OctezNode::spawn(config.octez_node_config.clone()).await?,
        })
    }

    async fn kill(&mut self) -> Result<()> {
        self.octez_node.kill().await
    }

    async fn health_check(&self) -> Result<bool> {
        self.octez_node.health_check().await
    }
}

pub struct JstzdServer {
    jstzd: Option<Jstzd>,
    jstzd_config: JstzdConfig,
    jstzd_server_port: u16,
    server_handle: Option<JoinHandle<()>>,
}

#[async_trait]
impl AsyncDrop for JstzdServer {
    async fn async_drop(&mut self) {
        if let Err(e) = self.stop().await {
            eprintln!("failed to stop jstzd server: {:?}", e);
        }
    }
}

impl JstzdServer {
    pub fn new(config: JstzdConfig, port: u16) -> Self {
        Self {
            jstzd: None,
            jstzd_config: config,
            jstzd_server_port: port,
            server_handle: None,
        }
    }

    pub async fn health_check(&self) -> bool {
        if let Some(v) = &self.server_handle {
            if !v.is_finished() {
                if let Some(jstzd) = &self.jstzd {
                    if let Ok(v) = jstzd.health_check().await {
                        return v;
                    }
                }
            }
        }

        false
    }

    pub async fn stop(&mut self) -> Result<()> {
        let mut err = None;
        if let Some(mut jstzd) = self.jstzd.take() {
            if let Err(e) = jstzd.kill().await {
                err.replace(e);
            };
        }
        if let Some(server) = self.server_handle.take() {
            server.abort();
        }
        match err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        self.jstzd
            .replace(Jstzd::spawn(self.jstzd_config.clone()).await?);

        let router = Router::new().route("/", axum::routing::get(http::StatusCode::OK));
        let listener = TcpListener::bind(("0.0.0.0", self.jstzd_server_port)).await?;

        let handle = tokio::spawn(async {
            axum::serve(listener, router).await.unwrap();
        });
        self.server_handle.replace(handle);
        Ok(())
    }
}
