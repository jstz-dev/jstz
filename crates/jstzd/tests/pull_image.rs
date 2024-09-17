use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use bollard::{image::ListImagesOptions, secret::ImageSummary, Docker};
use jstzd::docker::{GenericImage, Image};
use serial_test::serial;

// search image locally
async fn search_local_image(
    image_uri: String,
    client: Arc<Docker>,
) -> Result<Vec<ImageSummary>> {
    let filters = [("reference".to_string(), vec![image_uri])]
        .into_iter()
        .collect::<HashMap<_, _>>();
    let images = &client
        .list_images(Some(ListImagesOptions::<String> {
            all: true,
            filters,
            ..Default::default()
        }))
        .await;
    match images {
        Ok(images) => Ok(images.clone()),
        Err(_) => Err(anyhow::anyhow!("Image not found")),
    }
}
#[tokio::test]
#[serial]
async fn test_pull_image() -> Result<()> {
    let docker = Docker::connect_with_socket_defaults().unwrap();
    let docker = Arc::new(docker);
    let image = GenericImage::new("busybox").with_tag("stable");
    let _ = docker.remove_image(&image.image_uri(), None, None).await;
    image.pull_image(docker.clone()).await?;
    let expected_image_digest =
        "sha256:7db2ddde018a2a56e929855445bc7f30bc83db514a23404bd465a07d2770ac5f";
    let images = search_local_image(image.image_uri(), docker.clone()).await?;
    assert!(images.iter().any(|image| image.id == expected_image_digest));
    let _ = docker.remove_image(&image.image_uri(), None, None).await;
    Ok(())
}
