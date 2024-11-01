#[tokio::main]
async fn main() {
    if let Err(e) = jstzd::main().await {
        println!("hello");
        eprintln!("Error: {:?}", e);
        std::process::exit(1);
    }
}
