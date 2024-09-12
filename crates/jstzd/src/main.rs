use bollard::Docker;
use env_logger::Env;
use jstzd::docker::{GenericImage, Image};
use std::sync::Arc;

pub async fn example() -> anyhow::Result<()> {
    let docker = Docker::connect_with_socket_defaults().unwrap();
    let docker = Arc::new(docker);
    let image = GenericImage::new("busybox").with_tag("latest");
    image.pull_image(docker.clone()).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    match example().await {
        Ok(_) => log::info!("Success"),
        Err(e) => log::error!("Error: {}", e),
    }
}
