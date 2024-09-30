use std::path::PathBuf;

use anyhow::{bail, Result};
use tempfile::TempDir;

#[derive(Debug)]
pub enum Directory {
    TempDir(TempDir),
    Path(PathBuf),
}

impl TryFrom<PathBuf> for Directory {
    type Error = anyhow::Error;

    fn try_from(dir: PathBuf) -> Result<Self> {
        if !dir.exists() {
            bail!(format!(
                "Directory '{}' does not exist",
                dir.to_string_lossy()
            ));
        }

        if !dir.is_dir() {
            bail!(format!(
                "Path '{}' is not a directory",
                dir.to_string_lossy()
            ));
        }

        Ok(Directory::Path(dir))
    }
}

#[cfg(test)]
mod test {
    use tempfile::{NamedTempFile, TempDir};

    use super::Directory;

    #[test]
    fn test_directory_try_from_pathbuf() {
        // Directory does not exist
        let temp_file = NamedTempFile::new().unwrap();
        Directory::try_from(temp_file.path().to_path_buf())
            .expect_err("Should fail on file");

        // Path is not a directory
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        let _ = std::fs::remove_file(&dir_path);
        let _ = std::fs::remove_dir_all(&dir_path);
        Directory::try_from(dir_path).expect_err("Should fail when not directory");

        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        assert!(
            matches!(Directory::try_from(dir_path.clone()).unwrap(), Directory::Path(d) if d == dir_path),
        );
    }
}
