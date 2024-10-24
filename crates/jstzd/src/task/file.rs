use std::fs::File;
use std::path::PathBuf;

use anyhow::Result;
use tempfile::NamedTempFile;

#[derive(Debug)]
pub enum FileWrapper {
    TempFile(NamedTempFile),
    File((File, PathBuf)),
}

impl Default for FileWrapper {
    fn default() -> Self {
        Self::TempFile(NamedTempFile::new().unwrap())
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

    use tempfile::NamedTempFile;

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
}
