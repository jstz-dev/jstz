use std::collections::{btree_map::Entry, BTreeMap};

use anyhow::Result;
use jstz_runtime::wpt::{WptSubtest, WptSubtestStatus, WptTestStatus};
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

    pub fn stats(&self) -> BTreeMap<String, WptMetrics> {
        let mut stats = BTreeMap::new();
        for (folder, report) in self.test_harness() {
            for (suite_name, metrics) in report.stats() {
                let key = match suite_name.len() {
                    0 => folder.to_owned(),
                    _ => format!("{folder}/{suite_name}"),
                };
                stats.insert(key, metrics);
            }
        }
        stats
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

    pub fn stats(&self) -> BTreeMap<String, WptMetrics> {
        match self {
            WptReportFile::Folder(folder_map) => {
                let mut stats = BTreeMap::new();
                for (folder, report) in folder_map {
                    for (suite_name, metrics) in report.stats() {
                        let key = match suite_name.len() {
                            0 => folder.to_owned(),
                            _ => format!("{folder}/{suite_name}"),
                        };
                        stats.insert(key, metrics);
                    }
                }
                stats
            }
            WptReportFile::Test { variations } => BTreeMap::from_iter([(
                String::new(),
                variations
                    .iter()
                    .fold(WptMetrics::default(), |acc, report| {
                        let metrics = &report.metrics;
                        WptMetrics {
                            passed: acc.passed + metrics.passed,
                            failed: acc.failed + metrics.failed,
                            timed_out: acc.timed_out + metrics.timed_out,
                        }
                    }),
            )]),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct WptReportTest {
    pub subtests: Vec<WptSubtest>,
    pub status: WptTestStatus,
    pub metrics: WptMetrics,
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        WptMetrics, WptReport, WptReportFile, WptReportTest, WptSubtest,
        WptSubtestStatus, WptTestStatus,
    };

    fn report_file(passed: u64) -> WptReportFile {
        WptReportFile::Test {
            variations: vec![
                WptReportTest {
                    status: WptTestStatus::Ok,
                    subtests: vec![WptSubtest {
                        name: "foo".to_string(),
                        status: WptSubtestStatus::Pass,
                        message: None,
                    }],
                    metrics: WptMetrics {
                        passed,
                        failed: 0,
                        timed_out: 0,
                    },
                },
                WptReportTest {
                    status: WptTestStatus::Ok,
                    subtests: vec![WptSubtest {
                        name: "bar".to_string(),
                        status: WptSubtestStatus::Pass,
                        message: None,
                    }],
                    metrics: WptMetrics {
                        passed: 0,
                        failed: 2,
                        timed_out: 0,
                    },
                },
                WptReportTest {
                    status: WptTestStatus::Ok,
                    subtests: vec![WptSubtest {
                        name: "baz".to_string(),
                        status: WptSubtestStatus::Pass,
                        message: None,
                    }],
                    metrics: WptMetrics {
                        passed: 0,
                        failed: 0,
                        timed_out: 3,
                    },
                },
            ],
        }
    }

    #[test]
    fn wpt_report_file_stats_single_test_suite() {
        let report = report_file(1);
        let stats = report.stats();
        assert_eq!(
            stats,
            BTreeMap::from_iter([(
                "".to_string(),
                WptMetrics {
                    passed: 1,
                    failed: 2,
                    timed_out: 3,
                }
            )])
        );
    }

    #[test]
    fn wpt_report_file_stats_report_folder() {
        let report = WptReportFile::Folder(BTreeMap::from_iter([
            ("foo".to_string(), report_file(1)),
            ("bar".to_string(), report_file(100)),
            (
                "baz".to_string(),
                WptReportFile::Folder(BTreeMap::from_iter([
                    ("aaa".to_string(), report_file(5)),
                    ("bbb".to_string(), report_file(7)),
                ])),
            ),
        ]));
        assert_eq!(
            report.stats(),
            BTreeMap::from_iter([
                (
                    "foo".to_string(),
                    WptMetrics {
                        passed: 1,
                        failed: 2,
                        timed_out: 3,
                    }
                ),
                (
                    "bar".to_string(),
                    WptMetrics {
                        passed: 100,
                        failed: 2,
                        timed_out: 3,
                    }
                ),
                (
                    "baz/aaa".to_string(),
                    WptMetrics {
                        passed: 5,
                        failed: 2,
                        timed_out: 3,
                    }
                ),
                (
                    "baz/bbb".to_string(),
                    WptMetrics {
                        passed: 7,
                        failed: 2,
                        timed_out: 3,
                    }
                )
            ])
        );
    }

    #[test]
    fn wpt_report_stats() {
        let report = WptReport {
            test_harness: BTreeMap::from_iter([
                (
                    "test1".to_string(),
                    WptReportFile::Folder(BTreeMap::from_iter([
                        ("foo".to_string(), report_file(1)),
                        (
                            "bar".to_string(),
                            WptReportFile::Folder(BTreeMap::from_iter([
                                ("aaa".to_string(), report_file(5)),
                                ("bbb".to_string(), report_file(7)),
                            ])),
                        ),
                    ])),
                ),
                ("test2".to_string(), report_file(11)),
            ]),
        };
        assert_eq!(
            report.stats(),
            BTreeMap::from_iter([
                (
                    "test1/foo".to_string(),
                    WptMetrics {
                        passed: 1,
                        failed: 2,
                        timed_out: 3,
                    }
                ),
                (
                    "test1/bar/aaa".to_string(),
                    WptMetrics {
                        passed: 5,
                        failed: 2,
                        timed_out: 3,
                    }
                ),
                (
                    "test1/bar/bbb".to_string(),
                    WptMetrics {
                        passed: 7,
                        failed: 2,
                        timed_out: 3,
                    }
                ),
                (
                    "test2".to_string(),
                    WptMetrics {
                        passed: 11,
                        failed: 2,
                        timed_out: 3,
                    }
                ),
            ])
        );
    }
}
