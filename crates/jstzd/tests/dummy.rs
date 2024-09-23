use bollard::{Docker, API_DEFAULT_VERSION};
use jstzd::main;

#[tokio::test]
async fn test_main() {
    env_logger::init();

    let docker = Docker::connect_with_unix(
        "unix:///var/run/docker.sock",
        120,
        API_DEFAULT_VERSION,
    )
    .unwrap();
    let version = docker.version().await.unwrap();
    println!("{:?}", version);

    main().await.unwrap();
}
