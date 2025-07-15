use std::{ffi::OsStr, fmt::Display, path::PathBuf};

use anyhow::{bail, Result};
use serde_with::SerializeDisplay;
use tempfile::TempDir;

#[derive(Debug, SerializeDisplay)]
pub enum Directory {
    TempDir(TempDir),
    Path(PathBuf),
}

impl Display for Directory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Directory::Path(p) => p.to_string_lossy(),
            Directory::TempDir(p) => p.path().to_string_lossy(),
        };
        write!(f, "{s}")
    }
}

impl Default for Directory {
    fn default() -> Self {
        Self::TempDir(TempDir::new().unwrap())
    }
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

impl AsRef<OsStr> for Directory {
    fn as_ref(&self) -> &std::ffi::OsStr {
        match self {
            Directory::TempDir(temp_dir) => temp_dir.path().as_os_str(),
            Directory::Path(path) => path.as_os_str(),
        }
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

    #[test]
    fn test_from_directory() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        let directory = Directory::Path(dir_path.clone());
        let path_buf: PathBuf = (&directory).into();
        assert_eq!(path_buf, dir_path);
    }

    #[test]
    fn serialize() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        let directory = Directory::Path(dir_path.clone());
        assert_eq!(
            serde_json::to_value(&directory).unwrap(),
            serde_json::json!(dir_path.to_string_lossy())
        );

        let directory = Directory::TempDir(temp_dir);
        assert_eq!(
            serde_json::to_value(&directory).unwrap(),
            serde_json::json!(dir_path.to_string_lossy())
        );
    }

    #[test]
    fn display() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_path_buf();
        let directory = Directory::Path(dir_path.clone());
        assert_eq!(directory.to_string(), dir_path.to_str().unwrap());

        let directory = Directory::TempDir(temp_dir);
        assert_eq!(directory.to_string(), dir_path.to_str().unwrap());
    }
    #[test]
    fn test_directory_from_os_str() {
        // Test with valid UTF-8 path
        let valid_os_str = OsStr::new("/tmp/valid/path");
        let directory = Directory::Path(PathBuf::from(valid_os_str));
        assert_eq!(directory.to_string(), "/tmp/valid/path");

        // Test with non-UTF-8 path
        let invalid_bytes = b"/tmp/\xFF\xFE";
        let invalid_os_str = OsStr::from_bytes(invalid_bytes);
        let directory = Directory::Path(PathBuf::from(invalid_os_str));
        let res: Result<String, anyhow::Error> = (&directory).try_into();
        assert!(res.is_err());
    }
}
