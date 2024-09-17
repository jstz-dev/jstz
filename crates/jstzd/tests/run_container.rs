use std::sync::Arc;

use anyhow::Result;
use bollard::{container::ListContainersOptions, Docker};
use jstzd::docker::{GenericImage, Image, RunnableImage};

#[tokio::test]
async fn test_run_container() -> Result<()> {
    let docker = Docker::connect_with_socket_defaults().unwrap();
    let docker = Arc::new(docker);
    let image = GenericImage::new("busybox").with_tag("stable");
    image.pull_image(docker.clone()).await?;

    let runnable_image = RunnableImage::new(image.clone(), "busybox_test");
    let container = runnable_image.create_container(docker.clone()).await?;
    container.start().await?;
    let containers = docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await?;

    assert!(containers
        .iter()
        .any(|c| c.id.as_deref() == Some(&container.id)));

    let _ = docker.remove_image(&image.image_uri(), None, None).await;
    Ok(())
}
