use bollard::{Docker, API_DEFAULT_VERSION};
use jstzd::main;

#[tokio::test]
async fn test_main() {
    env_logger::init();

    println!(
        "current user: {:?} ({})",
        users::get_current_username(),
        users::get_current_uid()
    );

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
