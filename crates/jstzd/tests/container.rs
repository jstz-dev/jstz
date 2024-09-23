use std::sync::Arc;

use anyhow::Result;
use bollard::{container::ListContainersOptions, Docker};
use futures_util::AsyncBufReadExt;
use jstzd::docker::{GenericImage, Image, RunnableImage};
use serial_test::serial;

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn creates_container() -> Result<()> {
    let docker = docker();
    let image = image(docker.clone()).await?;
    let runnable_image = RunnableImage::new(image.clone(), "test_container1");
    let container = runnable_image.create_container(docker.clone()).await?;
    let option = Some(ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    });
    assert!(container_exists(docker, &container.id, option).await);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn runs_container() -> Result<()> {
    let docker = docker();
    let image = image(docker.clone()).await?;
    let runnable_image = RunnableImage::new(image.clone(), "test_container2");
    let container = runnable_image.create_container(docker.clone()).await?;
    container.start().await?;
    assert!(container_exists(docker, &container.id, None).await);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn container_streams_stdout() -> Result<()> {
    let docker = docker();
    let image = image(docker.clone()).await?;
    let cmd = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "sleep 1; echo hello from stdout;".to_string(),
    ];
    let runnable_image =
        RunnableImage::new(image.clone(), "test_container3").with_overridden_cmd(cmd);
    let container = runnable_image.create_container(docker.clone()).await?;
    container.start().await?;
    let mut stdout = container.stdout().await?;
    let mut out_line: String = String::new();
    stdout.read_line(&mut out_line).await?;
    assert_eq!(out_line, "hello from stdout\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn container_streams_stderr() -> Result<()> {
    let docker = docker();
    let image = image(docker.clone()).await?;
    let cmd = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "sleep 1; echo 'hello from stderr' >&2;".to_string(),
    ];
    let runnable_image =
        RunnableImage::new(image.clone(), "test_container3").with_overridden_cmd(cmd);
    let container = runnable_image.create_container(docker.clone()).await?;
    container.start().await?;
    let mut stderr = container.stderr().await?;
    let mut err_line: String = String::new();
    stderr.read_line(&mut err_line).await?;
    assert_eq!(err_line, "hello from stderr\n");
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn stdout_fails_for_container_not_running() -> Result<()> {
    let docker = docker();
    let image = image(docker.clone()).await?;
    let runnable_image = RunnableImage::new(image.clone(), "test_container3");
    let container = runnable_image.create_container(docker.clone()).await?;
    container.start().await?;
    let stdout = container.stdout().await;
    assert!(stdout.is_err());
    assert_eq!(
        stdout.err().unwrap().to_string(),
        "Container is not running".to_string()
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn removing_container_twice_fails() -> Result<()> {
    let docker = docker();
    let image = image(docker.clone()).await?;
    let runnable_image = RunnableImage::new(image.clone(), "test_container4");
    let container = runnable_image.create_container(docker.clone()).await?;
    container.start().await?;
    container.stop().await?;
    container.remove().await?;
    let result = container.remove().await;
    assert!(result.is_err());
    assert_eq!(
        result.err().unwrap().to_string(),
        format!("Failed to remove non existent container: {}", container.id)
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn removing_running_container_fails() -> Result<()> {
    let docker = docker();
    let image = image(docker.clone()).await?;
    let cmd = vec!["sh".to_string(), "-c".to_string(), "sleep 1".to_string()];
    let runnable_image =
        RunnableImage::new(image.clone(), "test_container5").with_overridden_cmd(cmd);
    let container = runnable_image.create_container(docker.clone()).await?;
    container.start().await?;
    let result = container.remove().await;
    assert!(result.is_err());
    assert!(result
        .err()
        .unwrap()
        .to_string()
        .contains("stop the container before removing"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn stops_and_removes_container_on_drop() -> Result<()> {
    let docker = docker();
    let image = image(docker.clone()).await?;
    let runnable_image = RunnableImage::new(image.clone(), "test_container3");
    let container = runnable_image.create_container(docker.clone()).await?;
    let container_id = container.id.clone();
    container.start().await?;
    std::mem::drop(container);
    let options = Some(ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    });
    assert!(!container_exists(docker, &container_id, options).await);
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn running_container_with_no_image_fails() -> Result<()> {
    let docker = docker();
    let image = image(docker.clone()).await?;
    docker.remove_image(&image.image_uri(), None, None).await?;
    let runnable_image = RunnableImage::new(image.clone(), "test_container6");
    let result = runnable_image.create_container(docker.clone()).await;
    assert!(result.is_err());
    assert_eq!(
        result.err().unwrap().to_string(),
        format!(
            "Image: {} not found, please make sure the image is pulled",
            image.image_uri()
        )
    );
    Ok(())
}

fn docker() -> Arc<Docker> {
    Arc::new(Docker::connect_with_socket_defaults().unwrap())
}

async fn image(client: Arc<Docker>) -> Result<GenericImage> {
    let image = GenericImage::new("busybox").with_tag("stable");
    image.pull_image(client).await?;
    Ok(image)
}

async fn container_exists(
    client: Arc<Docker>,
    container_id: &str,
    options: Option<ListContainersOptions<String>>,
) -> bool {
    let containers = client
        .list_containers(options.or(Some(ListContainersOptions::<String> {
            ..Default::default()
        })))
        .await
        .unwrap();
    containers
        .iter()
        .any(|c| c.id.as_deref() == Some(container_id))
}
