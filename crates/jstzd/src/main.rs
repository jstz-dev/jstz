use bollard::Docker;
use env_logger::Env;
use futures_util::{io::BufReader, AsyncBufReadExt};
use jstzd::docker::{Container, GenericImage, Image, RunnableImage};
use std::{process::Command, sync::Arc};

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

pub async fn example_stderr() -> anyhow::Result<()> {
    let docker = Docker::connect_with_socket_defaults().unwrap();
    let docker = Arc::new(docker);
    let image = GenericImage::new("busybox").with_tag("latest");
    image.pull_image(docker.clone()).await?;
    let cmd = vec![
        "/bin/sh",
        "-c",
        "echo 'HELLO STDOUT'",
        "echo 'HELLO STDERR' >&2",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let runnable_image =
        RunnableImage::new(image, "busybox_test").with_overridden_cmd(cmd);
    let id = runnable_image.create_container(docker.clone()).await?;
    let container = Container::new(docker.clone(), id);

    container.start().await?;
    let stdout = container.stdout().await?;
    let mut out_reader = BufReader::new(stdout);
    let mut out_line = String::new();
    while out_reader.read_line(&mut out_line).await? > 0 {
        println!("yo{}", out_line);
        out_line.clear();
    }

    // let stderr = container.stderr().await?;
    // let mut err_reader = BufReader::new(stderr);
    // let mut err_line = String::new();
    // while err_reader.read_line(&mut err_line).await? > 0 {
    //     println!("hey{}", err_line);
    //     err_line.clear();
    // }

    Ok(())
}

pub async fn exec_command_example() -> anyhow::Result<()> {
    let docker = Docker::connect_with_socket_defaults().unwrap();
    let docker = Arc::new(docker);
    let image = GenericImage::new("busybox").with_tag("stable");
    image.pull_image(docker.clone()).await?;
    let cmd = vec!["sleep".to_string(), "3600".to_string()];
    let runnable_image =
        RunnableImage::new(image, "busybox_test").with_overridden_cmd(cmd);
    let id = runnable_image.create_container(docker.clone()).await?;
    let container = Container::new(docker.clone(), id);
    container.start().await?;
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg("echo 'executing inside the container' >&2");
    let mut exec_result = container.exec(command).await?;
    let mut stdout = exec_result.stderr()?;
    let mut line = String::new();
    while stdout.read_line(&mut line).await? > 0 {
        println!("{}", line);
        line.clear(); // Clear the buffer for next line
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    match exec_command_example().await {
        Ok(_) => log::info!("Success"),
        Err(e) => log::error!("Error: {}", e),
    }
}
