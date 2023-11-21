use std::collections::{btree_map::Entry, BTreeMap};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct WptReport {
    test_harness: WptReportFolder,
}

impl WptReport {
    pub fn test_harness(&self) -> &WptReportFolder {
        &self.test_harness
    }

    pub fn insert(&mut self, path: &str, test: WptReportTest) -> Result<()> {
        insert_test_in_folder(&mut self.test_harness, path, test)
    }
}

#[derive(Default, Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct WptMetrics {
    pub passed: u64,
    pub failed: u64,
    pub timed_out: u64,
}

impl WptMetrics {
    pub fn total(&self) -> u64 {
        self.passed + self.failed + self.timed_out
    }
}

pub type WptReportFolder = BTreeMap<String, WptReportFile>;

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub enum WptReportFile {
    Folder(WptReportFolder),
    Test { variations: Vec<WptReportTest> },
}

fn insert_test_in_folder(
    folder: &mut WptReportFolder,
    path: &str,
    test: WptReportTest,
) -> Result<()> {
    // If this is the last path component, insert the test
    // Otherwise, recurse into the folder
    // If the folder doesn't exist, create it

    let components: Vec<&str> = path.split('/').collect();

    let filename = components[0];
    let remaining_path = components[1..].join("/");

    let entry = folder.entry(filename.to_string());

    match entry {
        Entry::Vacant(vacant_entry) if components.len() == 1 => {
            // This is the last path component, insert the test
            vacant_entry.insert(WptReportFile::Test {
                variations: vec![test],
            });
            Ok(())
        }
        Entry::Vacant(vacant_entry) => {
            // Create the folder if it doesn't exist (and recurse)
            vacant_entry
                .insert(WptReportFile::Folder(WptReportFolder::default()))
                .insert(&remaining_path, test)
        }
        Entry::Occupied(occupied_entry) => {
            // Recurse into the folder
            occupied_entry.into_mut().insert(&remaining_path, test)
        }
    }
}

impl WptReportFile {
    pub fn insert(&mut self, path: &str, test: WptReportTest) -> Result<()> {
        match self {
            WptReportFile::Folder(folder) => insert_test_in_folder(folder, path, test),
            WptReportFile::Test { variations } => {
                if !path.is_empty() {
                    return Err(anyhow::anyhow!("Attempted to folder into a test"));
                }

                variations.push(test);
                Ok(())
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct WptReportTest {
    pub subtests: Vec<WptSubtest>,
    pub status: WptTestStatus,
    pub metrics: WptMetrics,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub enum WptSubtestStatus {
    Pass = 0,
    Fail = 1,
    Timeout = 2,
    NotRun = 3,
    PreconditionFailed = 4,
}

impl TryFrom<u8> for WptSubtestStatus {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Pass),
            1 => Ok(Self::Fail),
            2 => Ok(Self::Timeout),
            3 => Ok(Self::NotRun),
            4 => Ok(Self::PreconditionFailed),
            _ => Err(()),
        }
    }
}

impl WptReportTest {
    pub fn new(status: WptTestStatus, subtests: Vec<WptSubtest>) -> Self {
        let mut metrics = WptMetrics::default();

        for subtest in &subtests {
            match subtest.status {
                WptSubtestStatus::Pass => metrics.passed += 1,
                WptSubtestStatus::Timeout => metrics.timed_out += 1,
                _ => metrics.failed += 1,
            }
        }

        Self {
            subtests,
            status,
            metrics,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct WptSubtest {
    pub name: String,
    pub status: WptSubtestStatus,
    pub message: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum WptTestStatus {
    Ok = 0,
    Err = 1,
    Timeout = 2,
    PreconditionFailed = 3,
    Null = 4,
}

impl TryFrom<u8> for WptTestStatus {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ok),
            1 => Ok(Self::Err),
            2 => Ok(Self::Timeout),
            3 => Ok(Self::PreconditionFailed),
            _ => Err(()),
        }
    }
}
