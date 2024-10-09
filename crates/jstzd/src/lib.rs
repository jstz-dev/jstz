pub mod docker;
pub mod jstzd;
pub mod task;

/// The `main` function for running jstzd
pub async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");
    Ok(())
}

pub fn unused_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
