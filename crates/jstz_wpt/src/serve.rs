use std::{borrow::Cow, future::IntoFuture, sync::Arc};

use anyhow::Result;
use nix::{
    sys::signal::{self, Signal::SIGINT},
    unistd::Pid,
};
use tl::Bytes;
use tokio::{process::Child, sync::Mutex};
use url::Url;

use crate::{
    manifest::WptManifest,
    report::{WptReport, WptReportTest},
    TestFilter, TestToRun,
};

const TEST_HARNESS_REPORT_PATH: &str = "/resources/testharnessreport.js";

pub type Script = String;

/// A bundle of scripts obtained from a web platform test
#[derive(Default, Debug, Clone)]
pub struct Bundle {
    pub items: Vec<BundleItem>,
}

#[derive(Debug, Clone)]
pub enum BundleItem {
    Resource(String, Script),
    TestHarnessReport,
    Inline(Script),
}

impl Bundle {
    pub fn push_resource(&mut self, location: &str, script: Script) {
        // Handle `testharnessreport.js` specially
        if location == TEST_HARNESS_REPORT_PATH {
            self.items.push(BundleItem::TestHarnessReport);
            return;
        }

        self.items
            .push(BundleItem::Resource(location.to_string(), script))
    }

    pub fn push_inline(&mut self, script: Script) {
        self.items.push(BundleItem::Inline(script))
    }
}

/// WptServe is a struct that represents a running wpt server
/// after running `wpt serve` in the wpt directory.
///
/// To stop the server, simply drop [`WptServe`] or call [`WptServe::kill`].
#[derive(Clone)]
pub struct WptServe {
    process: Arc<Mutex<Child>>,
    pid: i32,
}

impl WptServe {
    pub(crate) fn new(process: Child) -> Result<Self> {
        if let Some(pid) = process.id() {
            Ok(Self {
                process: Arc::new(Mutex::new(process)),
                pid: pid as i32,
            })
        } else {
            Err(anyhow::anyhow!("Failed to get process id"))
        }
    }

    pub async fn wait(&mut self) -> Result<()> {
        self.process.lock().await.wait().await?;
        Ok(())
    }
}

impl Drop for WptServe {
    fn drop(&mut self) {
        self.kill().expect("Failed to kill wpt server");
    }
}

pub type WptTestRunner<'a, R> = fn(&'a WptServe, TestToRun) -> R;

impl WptServe {
    /// Kill the wpt server
    pub fn kill(&self) -> Result<()> {
        let _ = signal::kill(Pid::from_raw(self.pid), SIGINT);

        Ok(())
    }

    // Fetch a resource
    async fn resource(&self, resource: &str) -> Result<String> {
        let res = reqwest::get(resource).await?;
        let body = res.text().await?;

        Ok(body)
    }

    /// Determine if the server is running
    pub async fn is_running(&self) -> Result<bool> {
        match reqwest::get("http://localhost:8000").await {
            Ok(res) if res.status().is_success() => Ok(true),
            _ => Ok(false),
        }
    }

    /// Bundle a test file
    pub async fn bundle(&self, test: &str) -> Result<Bundle> {
        let url = Url::parse(&format!("http://localhost:8000/{}", test))?;

        let body = self.resource(url.as_str()).await?;

        enum Script<'a> {
            Inline(Cow<'a, str>),
            External(&'a Bytes<'a>),
        }

        let dom = tl::parse(&body, Default::default())?;
        let parser = dom.parser();
        let scripts = dom.nodes().iter().filter_map(|node| {
            let tag = node.as_tag()?;

            if tag.name() == "script" {
                match tag.attributes().get("src") {
                    Some(Some(src)) => Some(Script::External(src)),
                    Some(None) => None,
                    None => Some(Script::Inline(tag.inner_text(parser))),
                }
            } else {
                None
            }
        });

        let mut bundle = Bundle::default();
        for script in scripts {
            match script {
                Script::Inline(content) => bundle.push_inline(content.to_string()),
                Script::External(location) => {
                    let location = url.join(&location.as_utf8_str())?;

                    bundle.push_resource(
                        location.path(),
                        self.resource(location.as_str()).await?,
                    );
                }
            }
        }

        Ok(bundle)
    }

    /// Given a manifest and a filter, traverse the
    /// manifest and run the tests that match the filter.
    /// The function `f` is called for each test.
    /// Returns a report of the tests that were run.
    pub async fn run_test_harness<'a, F>(
        &'a self,
        manifest: &WptManifest,
        filter: &TestFilter,
        f: WptTestRunner<'a, F>,
    ) -> Result<WptReport>
    where
        F: IntoFuture<Output = Result<WptReportTest>> + 'a,
    {
        let tests = manifest.tests(filter);

        let mut report = WptReport::default();

        for test in tests {
            let test_report = f(self, test.clone()).await?;
            report.insert(&test.manifest_path, test_report)?;
        }

        Ok(report)
    }
}
