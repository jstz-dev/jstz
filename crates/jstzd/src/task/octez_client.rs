use super::{endpoint::Endpoint, octez_node::DEFAULT_RPC_ENDPOINT};
use anyhow::{bail, Result};
use http::Uri;
use std::{path::PathBuf, str::FromStr};
use tempfile::{tempdir, TempDir};

const DEFAULT_BINARY_PATH: &str = "octez-client";
#[derive(Default)]
pub struct OctezClientBuilder {
    // if None, use the binary in $PATH
    binary_path: Option<PathBuf>,
    // if None, use temp directory
    base_dir: Option<PathBuf>,
    // if None, use localhost:8732 (the default endpoint for octez-node)
    endpoint: Option<Endpoint>,
    disable_unsafe_disclaimer: bool,
}

impl OctezClientBuilder {
    pub fn new() -> Self {
        OctezClientBuilder::default()
    }

    pub fn set_binary_path(mut self, binary_path: PathBuf) -> Self {
        self.binary_path = Some(binary_path);
        self
    }

    pub fn set_base_dir(mut self, base_dir: PathBuf) -> Self {
        self.base_dir = Some(base_dir);
        self
    }

    pub fn set_endpoint(mut self, endpoint: Endpoint) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    pub fn set_disable_unsafe_disclaimer(
        mut self,
        disable_unsafe_disclaimer: bool,
    ) -> Self {
        self.disable_unsafe_disclaimer = disable_unsafe_disclaimer;
        self
    }

    pub fn build(self) -> Result<OctezClient> {
        self.validate_binary_path()?;
        self.validate_base_dir()?;
        let node_default_endpoint = format!("http://{}", DEFAULT_RPC_ENDPOINT);
        Ok(OctezClient {
            binary_path: self.binary_path.unwrap_or(DEFAULT_BINARY_PATH.into()),
            base_dir: self
                .base_dir
                .map_or(Directory::TempDir(tempdir()?), Directory::Path),
            endpoint: self
                .endpoint
                .unwrap_or(Endpoint::try_from(Uri::from_str(&node_default_endpoint)?)?),
            disable_unsafe_disclaimer: self.disable_unsafe_disclaimer,
        })
    }

    fn validate_binary_path(&self) -> Result<()> {
        if let Some(binary_path) = &self.binary_path {
            if !binary_path.exists() {
                bail!("Binary path does not exist");
            }
            if !binary_path.is_file() {
                bail!("Binary path is not a file");
            }
        }
        Ok(())
    }

    fn validate_base_dir(&self) -> Result<()> {
        if let Some(base_dir) = &self.base_dir {
            if !base_dir.exists() {
                bail!("Base directory does not exist");
            }
            if !base_dir.is_dir() {
                bail!("Base directory is not a directory");
            }
        }
        Ok(())
    }
}

enum Directory {
    TempDir(TempDir),
    Path(PathBuf),
}

#[allow(dead_code)]
pub struct OctezClient {
    binary_path: PathBuf,
    base_dir: Directory,
    endpoint: Endpoint,
    disable_unsafe_disclaimer: bool,
}

#[cfg(test)]
mod test {
    use tempfile::NamedTempFile;

    use super::*;
    #[test]
    fn builds_default_octez_client() {
        let octez_client = OctezClientBuilder::new().build().unwrap();
        assert_eq!(
            octez_client.binary_path.to_str().unwrap(),
            DEFAULT_BINARY_PATH
        );
        assert!(matches!(octez_client.base_dir, Directory::TempDir(_)));
        assert!(!octez_client.disable_unsafe_disclaimer);
        assert_eq!(octez_client.endpoint, Endpoint::localhost(8732))
    }

    #[test]
    fn temp_dir_is_created_by_default() {
        let octez_client = OctezClientBuilder::new().build().unwrap();
        assert!(
            matches!(octez_client.base_dir, Directory::TempDir(temp_dir) if temp_dir.path().exists())
        );
    }

    #[test]
    fn temp_dir_is_removed_on_drop() {
        let octez_client = OctezClientBuilder::new().build().unwrap();
        match &octez_client.base_dir {
            Directory::TempDir(temp_dir) => {
                let temp_dir_path = temp_dir.path().to_path_buf();
                drop(octez_client);
                assert!(!temp_dir_path.exists());
            }
            _ => panic!("Expected TempDir"),
        };
    }

    #[test]
    fn sets_custom_binary_and_dir_path() {
        let temp_file = NamedTempFile::new().unwrap();
        let binary_path = temp_file.path().to_path_buf();
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        let octez_client = OctezClientBuilder::new()
            .set_binary_path(binary_path.clone())
            .set_base_dir(dir_path.clone())
            .build()
            .unwrap();
        assert_eq!(octez_client.binary_path, binary_path);
        assert!(
            matches!(octez_client.base_dir, Directory::Path(path) if path == dir_path)
        );
    }

    #[test]
    fn validates_base_dir_exists() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        let _ = std::fs::remove_file(&dir_path);
        let _ = std::fs::remove_dir_all(&dir_path);
        let octez_client = OctezClientBuilder::new()
            .set_base_dir(dir_path.clone())
            .build();
        assert!(octez_client
            .is_err_and(|e| &e.to_string() == "Base directory does not exist"));
    }

    #[test]
    fn validates_base_dir_is_dir() {
        let temp_file = NamedTempFile::new().unwrap();
        let invalid_dir_path = temp_file.path().to_path_buf();
        let octez_client = OctezClientBuilder::new()
            .set_base_dir(invalid_dir_path.clone())
            .build();
        assert!(octez_client
            .is_err_and(|e| &e.to_string() == "Base directory is not a directory"));
    }

    #[test]
    fn validates_binary_path_exists() {
        let temp_file = NamedTempFile::new().unwrap();
        let binary_path = temp_file.path().to_path_buf();
        let _ = std::fs::remove_file(&binary_path);
        let _ = std::fs::remove_dir_all(&binary_path);
        let octez_client = OctezClientBuilder::new()
            .set_binary_path(binary_path.clone())
            .build();
        assert!(
            octez_client.is_err_and(|e| &e.to_string() == "Binary path does not exist")
        );
    }
    #[test]
    fn validates_binary_path_is_file() {
        let temp_dir = TempDir::new().unwrap();
        let invalid_binary_path = temp_dir.path().to_path_buf();
        let octez_client = OctezClientBuilder::new()
            .set_binary_path(invalid_binary_path.clone())
            .build();
        assert!(
            octez_client.is_err_and(|e| &e.to_string() == "Binary path is not a file")
        );
    }
}
