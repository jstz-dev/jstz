use bollard::Docker;
use jstzd::main;

#[tokio::test]
async fn test_main() {
    let docker = Docker::connect_with_unix_defaults().unwrap();
    let version = docker.version().await.unwrap();
    println!("{:?}", version);

    main().await.unwrap();
}
