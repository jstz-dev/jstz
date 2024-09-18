#[tokio::main]
async fn main() {
    if let Err(e) = jstzd::main().await {
        eprintln!("Error: {:?}", e);
        std::process::exit(1);
    }
}
