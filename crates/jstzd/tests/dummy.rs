use jstzd::main;
use tokio::process::Command;

#[tokio::test]
async fn test_main() {
    main().await.unwrap();
}

#[tokio::test]
async fn test_octez_client() {
    let output = Command::new("octez-client")
        .arg("--version")
        .output()
        .await
        .unwrap();
    println!("octez-client --version: {:?}", output);
}
