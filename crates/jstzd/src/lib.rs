pub mod docker;
pub mod task;

/// The `main` function for running jstzd
pub async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");
    Ok(())
}
