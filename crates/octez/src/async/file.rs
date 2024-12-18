use std::fmt::Display;
use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
use serde_with::SerializeDisplay;
use tempfile::NamedTempFile;

#[derive(Debug, SerializeDisplay)]
pub enum FileWrapper {
    TempFile(NamedTempFile),
    File((File, PathBuf)),
}

impl Default for FileWrapper {
    fn default() -> Self {
        Self::TempFile(NamedTempFile::new().unwrap())
    }
}

impl PartialEq for FileWrapper {
    fn eq(&self, other: &Self) -> bool {
        match self {
            FileWrapper::File((_, p1)) => match other {
                FileWrapper::File((_, p2)) => p1 == p2,
                _ => false,
            },
            FileWrapper::TempFile(v1) => match other {
                FileWrapper::TempFile(v2) => v1.path() == v2.path(),
                _ => false,
            },
        }
    }
}

impl TryFrom<PathBuf> for FileWrapper {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> {
        Ok(FileWrapper::File((
            File::options()
                .read(true)
                .write(true)
                .truncate(true)
                .create(true)
                .open(&path)?,
            path,
        )))
    }
}

impl Display for FileWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            FileWrapper::File(p) => p.1.to_string_lossy(),
            FileWrapper::TempFile(p) => p.path().to_string_lossy(),
        };
        write!(f, "{}", s)
    }
}

impl FileWrapper {
    pub fn as_file(&self) -> &File {
        match self {
            FileWrapper::File((v, _)) => v,
            FileWrapper::TempFile(v) => v.as_file(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        path::PathBuf,
    };

    use tempfile::{NamedTempFile, TempPath};

    use super::FileWrapper;

    #[test]
    fn file_try_from_pathbuf_invalid_path() {
        FileWrapper::try_from(PathBuf::from("/foo/bar"))
            .expect_err("Should fail to create a file with an invalid path");
    }

    #[test]
    fn file_try_from_pathbuf_open_existing_file() {
        let mut file = NamedTempFile::new().unwrap();
        let path = PathBuf::from(&file.path().to_str().unwrap());
        assert!(path.exists());
        file.write_all("foobar".as_bytes()).unwrap();

        let f = FileWrapper::try_from(path).unwrap();
        match f {
            FileWrapper::File((mut f, v)) => {
                assert!(v.exists());
                let mut s = String::new();
                f.read_to_string(&mut s).unwrap();
                // file is truncated
                assert!(s.is_empty());
            }
            _ => panic!("should be a path"),
        }
    }

    #[test]
    fn file_try_from_pathbuf_create_new_file() {
        let tmp_path =
            PathBuf::from(NamedTempFile::new().unwrap().path().to_str().unwrap());
        assert!(!tmp_path.exists());
        let f = FileWrapper::try_from(tmp_path.clone()).unwrap();
        match f {
            FileWrapper::File((_, v)) => {
                assert!(v.exists());
            }
            _ => panic!("should be a path"),
        }
    }

    #[test]
    fn file_default_create_temp_file() {
        match FileWrapper::default() {
            FileWrapper::TempFile(v) => {
                assert!(PathBuf::from(v.path()).exists());
            }
            _ => panic!("should be a temp file"),
        }
    }

    #[test]
    fn display_file() {
        let path = NamedTempFile::new().unwrap().into_temp_path();
        let file = FileWrapper::try_from(path.to_path_buf()).unwrap();
        assert_eq!(file.to_string(), serde_json::json!(path.to_string_lossy()));
    }

    #[test]
    fn display_tempfile() {
        let tmp_file = NamedTempFile::new().unwrap();
        let path = tmp_file.path().to_path_buf();
        let expected = path.to_str().unwrap();
        let file = FileWrapper::TempFile(tmp_file);
        assert_eq!(file.to_string(), expected);
    }

    #[test]
    fn serialize_file() {
        let path = NamedTempFile::new().unwrap().into_temp_path();
        let file = FileWrapper::try_from(path.to_path_buf()).unwrap();
        assert_eq!(
            serde_json::to_value(&file).unwrap(),
            serde_json::json!(path.to_string_lossy())
        );
    }

    #[test]
    fn serialize_tempfile() {
        let tmp_file = NamedTempFile::new().unwrap();
        let expected = serde_json::json!(tmp_file.path().to_string_lossy());
        let file = FileWrapper::TempFile(tmp_file);
        let serialized = serde_json::to_value(&file).unwrap();
        assert_eq!(serialized, expected);
    }

    #[test]
    fn partial_eq() {
        let (tmp_file, tmp_path) = NamedTempFile::new().unwrap().into_parts();
        let file1 = FileWrapper::try_from(tmp_path.to_path_buf()).unwrap();
        let file2 = FileWrapper::try_from(tmp_path.to_path_buf()).unwrap();

        let path = tmp_path.to_path_buf();
        let file3 = FileWrapper::TempFile(NamedTempFile::from_parts(
            tmp_file.try_clone().unwrap(),
            TempPath::from_path(path.clone()),
        ));
        let file4 = FileWrapper::TempFile(NamedTempFile::from_parts(
            tmp_file.try_clone().unwrap(),
            TempPath::from_path(path),
        ));

        assert_eq!(file1, file1);
        assert_eq!(file1, file2);
        assert_eq!(file3, file4);
        assert_ne!(file2, file3);
    }
}
