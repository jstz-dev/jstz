use bollard::Docker;
use env_logger::Env;
use jstzd::docker::{Container, GenericImage, Image, RunnableImage};
use std::sync::Arc;

pub async fn example() -> anyhow::Result<()> {
    let docker = Docker::connect_with_socket_defaults().unwrap();
    let docker = Arc::new(docker);
    let image = GenericImage::new("busybox").with_tag("latest");
    image.pull_image(docker.clone()).await?;
    let cmd = vec![
        "sh",
        "-c",
        "for i in $(seq 1 3); do echo 'HELLO FROM INSIDE THE CONTAINER'; sleep 1; done",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let runnable_image =
        RunnableImage::new(image, "busybox_test").with_overridden_cmd(cmd);
    let id = runnable_image.create_container(docker.clone()).await?;
    let container = Container::new(docker.clone(), id);
    container.start().await?;

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
