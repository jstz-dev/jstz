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

impl TryFrom<&Directory> for String {
    type Error = anyhow::Error;
    fn try_from(dir: &Directory) -> Result<Self> {
        match dir {
            Directory::TempDir(temp_dir) => temp_dir
                .path()
                .to_str()
                .map(|s| s.to_string())
                .ok_or_else(|| {
                    anyhow::anyhow!("directory path must be a valid utf-8 path")
                }),
            Directory::Path(path) => {
                path.to_str().map(|s| s.to_string()).ok_or_else(|| {
                    anyhow::anyhow!("directory path must be a valid utf-8 path")
                })
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt, path::PathBuf};

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

    #[test]
    fn test_string_try_from_directory() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        let directory = Directory::Path(dir_path.clone());
        let res: Result<String, anyhow::Error> = (&directory).try_into();
        assert!(res.is_ok_and(|s| s == dir_path.to_string_lossy()));
    }

    #[test]
    fn test_string_try_from_directory_fails_on_invalid_path() {
        let invalid_bytes = b"/tmp/\xFF\xFE";
        let invalid_file_path = PathBuf::from(OsStr::from_bytes(invalid_bytes));
        let directory = Directory::Path(invalid_file_path);
        let res: Result<String, anyhow::Error> = (&directory).try_into();
        assert!(res.is_err_and(|e| { e.to_string().contains("valid utf-8 path") }));
    }
}
