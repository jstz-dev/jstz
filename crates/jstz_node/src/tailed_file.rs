use std::io::SeekFrom;
use tokio::io::{AsyncSeekExt, BufReader, Result};
use tokio::{fs::File, io::Lines};

pub struct TailedFile(BufReader<File>);

pub use tokio::io::AsyncBufReadExt;

impl TailedFile {
    pub async fn init(path: &str) -> Result<Self> {
        let mut file = File::open(path).await?;
        let _ = file.seek(SeekFrom::End(0));
        let reader = BufReader::new(file);

        Ok(TailedFile(reader))
    }

    pub fn lines(self) -> Lines<BufReader<File>> {
        self.0.lines()
    }
}
