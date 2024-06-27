use std::{collections::BTreeMap, result};

use regex::RegexSet;
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

impl From<WptManifestTest> for internal::WptManifestTest {
    fn from(val: WptManifestTest) -> Self {
        let mut items = vec![internal::WptManifestTestItem::Hash(val.hash)];

        items.extend(
            val.variations
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

impl From<WptTestVariation> for (Option<String>, WptTestOptions) {
    fn from(val: WptTestVariation) -> Self {
        (val.path, val.options)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WptTestOptions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script_metadata: Vec<(String, String)>,
}

/// TestFilters provide a way to filter tests from [`WptManifest`]
#[derive(Default, Debug, Clone)]
pub struct TestFilter {
    pub folders: RegexSet,
}

impl TestFilter {
    /// Returns true if the path matches a folder in
    /// the filter
    pub fn is_match(&self, path: &str) -> bool {
        if self.folders.is_empty() {
            return true;
        }

        self.folders.is_match(path)
    }
}

impl TryFrom<&[&str]> for TestFilter {
    type Error = anyhow::Error;

    fn try_from(value: &[&str]) -> anyhow::Result<Self> {
        let folders = RegexSet::new(value)?;
        Ok(Self { folders })
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
                    let Some(url_path) = url_path else {
                        return None;
                    };

                    // We need to parse the path as a URL to get the path and query string (separately)
                    let url = Url::parse(&format!("http://localhost:8000/{}", url_path))
                        .ok()?;

                    if !filter.is_match(url.path()) {
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
