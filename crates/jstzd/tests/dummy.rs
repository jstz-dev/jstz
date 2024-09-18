use jstzd::main;

#[tokio::test]
async fn test_main() {
    main().await.unwrap();
}
