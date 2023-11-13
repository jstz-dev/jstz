use std::{collections::BTreeMap, result};

use serde::{Deserialize, Serialize};
use url::Url;

use self::internal::WptManifestTestItem;

/// WptManifest is a struct that represents the wpt manifest
///
/// The wpt manifest is a JSON file that contains a list of all the tests.
///
/// NOTE: This is a partial implementation of the wpt manifest --
/// it only contains the `testharness` folder.
#[derive(Debug, Serialize, Deserialize)]
pub struct WptManifest {
    #[serde(rename = "items")]
    pub folders: WptManifestFolders,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WptManifestFolders {
    #[serde(rename = "testharness")]
    pub test_harness: WptManifestFolder,
}

pub type WptManifestFolder = BTreeMap<String, WptManifestFile>;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum WptManifestFile {
    Folder(WptManifestFolder),
    Test(WptManifestTest),
}

mod internal {
    use super::*;

    pub type WptManifestTest = Vec<WptManifestTestItem>;

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum WptManifestTestItem {
        Hash(String),
        Variations(WptTestVariation),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    into = "internal::WptManifestTest",
    try_from = "internal::WptManifestTest"
)]
pub struct WptManifestTest {
    pub hash: String,
    pub variations: Vec<WptTestVariation>,
}

impl TryFrom<internal::WptManifestTest> for WptManifestTest {
    type Error = &'static str;

    fn try_from(value: internal::WptManifestTest) -> result::Result<Self, Self::Error> {
        let hash = match &value[0] {
            WptManifestTestItem::Hash(hash) => hash.clone(),
            _ => return Err("Expected hash as first item"),
        };

        let variations = value
            .into_iter()
            .skip(1)
            .map(|item| match item {
                WptManifestTestItem::Variations(variation) => Ok(variation),
                _ => Err("Expected test variations following the hash"),
            })
            .collect::<result::Result<Vec<WptTestVariation>, Self::Error>>()?;

        Ok(Self {
            hash: hash.clone(),
            variations,
        })
    }
}

impl Into<internal::WptManifestTest> for WptManifestTest {
    fn into(self) -> internal::WptManifestTest {
        let mut items = vec![internal::WptManifestTestItem::Hash(self.hash)];

        items.extend(
            self.variations
                .into_iter()
                .map(internal::WptManifestTestItem::Variations),
        );

        items
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    from = "(Option<String>, WptTestOptions)",
    into = "(Option<String>, WptTestOptions)"
)]
pub struct WptTestVariation {
    pub path: Option<String>,
    pub options: WptTestOptions,
}

impl From<(Option<String>, WptTestOptions)> for WptTestVariation {
    fn from((path, options): (Option<String>, WptTestOptions)) -> Self {
        Self { path, options }
    }
}

impl Into<(Option<String>, WptTestOptions)> for WptTestVariation {
    fn into(self) -> (Option<String>, WptTestOptions) {
        (self.path, self.options)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WptTestOptions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script_metadata: Vec<(String, String)>,
}

/// TestFilters provide a way to filter tests from [`WptManifest`]
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct TestFilter {
    pub folders: Vec<String>,
}

impl TestFilter {
    /// Returns true if the path matches a folder in
    /// the filter
    pub fn matches(&self, path: &str) -> bool {
        println!("Matching path: {}", path);

        if self.folders.is_empty() {
            return true;
        }

        self.folders.iter().any(|folder| path.starts_with(folder))
    }
}

impl From<&[&str]> for TestFilter {
    fn from(value: &[&str]) -> Self {
        Self {
            folders: value.iter().map(|&str| String::from(str)).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestToRun {
    pub manifest_path: String,
    pub url_path: String,
    pub options: WptTestOptions,
}

pub trait WptTests {
    fn tests(&self, path: String, filter: &TestFilter) -> Vec<TestToRun>;
}

impl WptTests for WptManifestTest {
    fn tests(&self, path: String, filter: &TestFilter) -> Vec<TestToRun> {
        self.variations
            .iter()
            .filter_map(
                |WptTestVariation {
                     path: url_path,
                     options,
                 }|
                 -> Option<TestToRun> {
                    let Some(url_path) = url_path else { return None };

                    println!("url_path: {}", url_path);

                    // We need to parse the path as a URL to get the path and query string (separately)
                    let url = Url::parse(&format!("http://localhost:8000/{}", url_path))
                        .ok()?;

                    if !filter.matches(url.path()) {
                        return None;
                    }

                    // Our wpt library should only emit tests for:
                    //   - Tests running on any platform (.any.html)
                    //   - Tests running in a browser (.window.html)
                    //   - Tests running in a worker (.worker.html)
                    //   - Tests running in a worker module (.worker-module.html)
                    if !url.path().ends_with(".any.html")
                        && !url.path().ends_with(".window.html")
                        && !url.path().ends_with(".worker.html")
                        && !url.path().ends_with(".worker-module.html")
                    {
                        return None;
                    }

                    // Tests that require a HTTP2 compatible server are not supported
                    if url.path().contains(".h2.")
                        || url.path().contains("request-upload")
                    {
                        return None;
                    }

                    Some(TestToRun {
                        url_path: url_path.clone(),
                        manifest_path: path.clone(),
                        options: options.clone(),
                    })
                },
            )
            .collect()
    }
}

impl WptTests for WptManifestFile {
    fn tests(&self, path: String, filter: &TestFilter) -> Vec<TestToRun> {
        match self {
            WptManifestFile::Folder(folder) => folder
                .iter()
                .flat_map(|(name, file)| file.tests(format!("{}/{}", path, name), filter))
                .collect(),
            WptManifestFile::Test(test) => test.tests(path, filter),
        }
    }
}

impl WptManifest {
    pub fn tests(&self, filter: &TestFilter) -> Vec<TestToRun> {
        self.folders
            .test_harness
            .iter()
            .flat_map(|(name, file)| file.tests(name.to_string(), filter))
            .collect()
    }
}
