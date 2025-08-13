use anyhow::Context;
use deno_core::{
    op2,
    v8::{self},
    OpState,
};
use deno_error::JsErrorBox;

#[path = "report_parser.rs"]
mod report_parser;
use report_parser::parse_report_from_log_line;

use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use jstz_runtime::wpt::init_runtime;
use jstz_runtime::wpt::test_completion_callback;
use jstz_runtime::wpt::test_result_callback;
use jstz_runtime::wpt::{
    LogLine, ParseError, TestHarnessReport, TestResult, TestsResult, WptSubtest,
    WptSubtestStatus, WptTestStatus,
};
use jstz_runtime::{JstzRuntime, JstzRuntimeOptions, RuntimeContext};
use jstz_wpt::{
    Bundle, BundleItem, TestFilter, TestToRun, Wpt, WptMetrics, WptReportTest, WptServe,
};
use regex::Regex;
use ron::de::from_str as ron_from_str;
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    future::IntoFuture,
    path::{Path, PathBuf},
    str::FromStr,
};
use tezos_smart_rollup_mock::MockHost;
use tokio::io::AsyncWriteExt;

/// List of test prefixes that should be skipped due to known issues
const SKIP_TESTS: &[&str] = &[
    "FileAPI/url/url-format.any.html",
    "compression/compression",
    "encoding/",
    "fetch/http-cache/",
    "webstorage/",
    "html/webappapis/scripting/processing-model-2",
];

fn should_skip_test(test_path: &str) -> bool {
    SKIP_TESTS
        .iter()
        .any(|&skip_path| test_path.contains(skip_path))
}

pub async fn run_wpt_test_harness(bundle: &Bundle) -> TestHarnessReport {
    let mut tx = Transaction::default();
    tx.begin();
    let mut host = MockHost::default();
    host.set_debug_handler(std::io::empty());

    let mut source = String::new();
    for item in &bundle.items {
        match item {
            BundleItem::TestHarnessReport => {
                // Register test callback
                source += "add_result_callback(globalThis.test_result_callback); add_completion_callback(globalThis.test_completion_callback);";
            }
            BundleItem::Inline(script) | BundleItem::Resource(_, script) => {
                source += script;
            }
        }
    }

    //println!("source: {}", source);
    //eprintln!("source: {}", source);

    // RUN NORMALLY
    /*let mut rt = init_runtime(&mut host, &mut tx);

    // Somehow each `execute_script` call has some strange side effect such that the global
    // test suite object is completed prematurely before all test cases are registered.
    // Therefore, instead of executing each piece of test scripts separately, we need to
    // collect them and run them all in one `execute_script` call.);

    // Use catch_unwind to handle panics (including segmentation faults) gracefully
    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        rt.execute_script("native code", source.clone())
    }));

    */
    let err_report = TestHarnessReport {
        status: Some(WptTestStatus::Err),
        subtests: vec![WptSubtest {
            name: "Script execution failed".to_string(),
            status: WptSubtestStatus::Fail,
            message: Some(
                "Test failed due to script execution error (panic/segfault)".to_string(),
            ),
        }],
    }; /*

       match result {
           Ok(_) => {
               //println!("script executed successfully");
           }
           Err(e) => {
               println!("wpt: script execution failed with panic: {:?}", e);
               // Return a default report indicating the test failed due to execution error
               return err_report;
           }
       }*/

    // \RUN NORMALLY

    // RUN IN RISCV
    // Call the external binary to create the message
    let output = std::process::Command::new("cargo")
        .args([
            "run",
            "--package",
            "jstz_message_creator",
            "--bin",
            "jstz_message_creator",
        ])
        .arg(source.clone())
        .output()
        .expect("Failed to execute jstz_message_creator binary");

    // Print the output from the binary
    if !output.stdout.is_empty() {
        /*println!(
            "jstz_message_creator stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );*/
    }
    if !output.stderr.is_empty() {
        /*eprintln!(
            "jstz_message_creator stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );*/
    }

    if !output.status.success() {
        println!(
            "jstz_message_creator failed with exit code: {}",
            output.status.code().unwrap_or(-1)
        );
        /*eprintln!(
            "jstz_message_creator failed with exit code: {}",
            output.status.code().unwrap_or(-1)
        );*/
        return TestHarnessReport {
            status: Some(WptTestStatus::Err),
            subtests: vec![WptSubtest {
                name: "Message creation failed".to_string(),
                status: WptSubtestStatus::Fail,
                message: Some("Failed to create message via external binary".to_string()),
            }],
        };
    }

    // \RUN IN RISCV

    //println!("Message created successfully via external binary");

    // Take the test harness report out of the runtime and return it
    // Need to store data temporarily so that the borrow can be dropped
    //let data = rt.op_state().borrow().borrow::<TestHarnessReport>().clone();
    let data = parse_report_from_log_line(
        format!("{}", String::from_utf8_lossy(&output.stdout)).as_str(),
    )
    .unwrap()
    .unwrap_or(err_report);

    println!("report: {:?}", data);
    data
}

fn process_subtests(url_path: &str, mut substests: Vec<WptSubtest>) -> Vec<WptSubtest> {
    let files = [
        // fetch related
        r".*\/request\-cache.*",
        r".*\/cache\.https\.any.*",
        r".*\/conditional\-get.*",
        r".*\/stale\-while\-revalidate",
        // misc
        r".*\/general\.any.*",
        // http
        r".*\/redirect\-count\.any.*",
        r".*\/http\-response\-code\.any.*",
        r".*\/http\-cache.*",
    ]
    .join("|");

    let re = Regex::new(files.as_str()).unwrap();
    if re.is_match(url_path) {
        substests.iter_mut().for_each(|subtest| {
            if subtest.status == WptSubtestStatus::Fail {
                subtest.message = Some("Message omitted to stabilize report".to_string())
            }
        });
    }

    substests
}

fn run_wpt_test(
    wpt_serve: &WptServe,
    test: TestToRun,
) -> impl IntoFuture<Output = anyhow::Result<WptReportTest>> + '_ {
    async move {
        println!("");
        println!("starting test {}", &test.url_path);

        // Check if this test should be skipped
        if should_skip_test(&test.url_path) {
            println!("skipping test {} (in skip list)", &test.url_path);
            return Ok(WptReportTest::new(
                WptTestStatus::Err,
                vec![WptSubtest {
                    name: "Test skipped".to_string(),
                    status: WptSubtestStatus::NotRun,
                    message: Some("Test skipped due to known issues".to_string()),
                }],
            ));
        }

        let bundle = match wpt_serve.bundle(&test.url_path).await {
            Ok(bundle) => bundle,
            Err(e) => {
                println!("failed to bundle test {}: {}", &test.url_path, e);
                return Ok(WptReportTest::new(
                    WptTestStatus::Err,
                    vec![WptSubtest {
                        name: "Bundle failed".to_string(),
                        status: WptSubtestStatus::Fail,
                        message: Some(format!("Failed to bundle test: {}", e)),
                    }],
                ));
            }
        };
        let report = run_wpt_test_harness(&bundle).await;
        println!("test {} => {:?}", &test.url_path, &report.status);
        // Each test suite should have a status code attached after it completes.
        // When unwrap fails, it means something is wrong, e.g. some tests failed because
        // of something not yet supported by the runtime, such that the test completion callback
        // was not even triggered and we should fix that.
        let status = report.status.clone().unwrap_or(WptTestStatus::Err);
        let subtests = report.subtests.clone();
        Ok(WptReportTest::new(
            status,
            process_subtests(&test.url_path, subtests),
        ))
    }
}

/// Content of the report file generated by Deno's WPT runner with the `--wptreport` parameter.
#[derive(Deserialize)]
struct DenoReport {
    results: Vec<DenoResult>,
}

impl DenoReport {
    pub fn test_paths(&self) -> Vec<String> {
        self.results.iter().map(|v| v.test.to_string()).collect()
    }

    pub fn load(path: PathBuf) -> anyhow::Result<Self> {
        let f = File::open(path).context("failed to open deno report file")?;
        serde_json::from_reader::<_, DenoReport>(f)
            .context("failed to deserialise deno report")
    }

    pub fn stats(&self) -> BTreeMap<String, (u64, u64)> {
        let mut map = BTreeMap::new();
        let base_url = url::Url::parse("http://host/").unwrap();
        for result in &self.results {
            let key = base_url
                .join(&result.test)
                .expect("should be able to parse test names (URL path to test suite)")
                // Only the path is used as the key here, which means subtests filtered by
                // query parameters are aggregated into the same test suite.
                .path()
                .to_string();
            let test_count = match result.subtests.len() {
                // If there is no subtest, the test suite itself is the test.
                0 => 1,
                v => v,
            } as u64;
            let expected_pass_count = match (&result.status, result.subtests.len()) {
                // If there is no subtest, the test suite itself is the test, and if the status
                // of the suite is OK, it means that this test suite itself works
                (DenoTestStatus::Ok, 0) => 1,
                // When a test suite has other statuses, it might mean that it completely failed
                // or partially completed, so we need to look through its subtests. If a test suite
                // does not have any subtest and its status is not OK, it means that it doesn't work
                // completely and we don't count it as an expected pass.
                _ => result.subtests.iter().fold(0, |acc, t| {
                    acc + match t.expected {
                        Some(DenoSubtestStatus::Fail) => 0,
                        _ => 1,
                    }
                }),
            };
            let value = match map.get(&key) {
                Some((total, passed)) => {
                    (test_count + total, expected_pass_count + passed)
                }
                None => (test_count, expected_pass_count),
            };
            map.insert(key, value);
        }
        map
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum DenoTestStatus {
    Ok,
    Fail,
    Error,
    Crash,
}

#[derive(Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum DenoSubtestStatus {
    Pass,
    Fail,
}

/// Result of a test suite.
#[derive(Deserialize)]
struct DenoResult {
    /// URL path to the test suite, e.g. `/xhr/send-send.any.worker.html`.
    test: String,
    subtests: Vec<DenoSubtestResult>,
    /// Status of the test suite.
    status: DenoTestStatus,
}

/// A subset of the subtest results reported by Deno's WPT runner. We only care about the expected
/// result of a test here.
#[derive(Deserialize)]
struct DenoSubtestResult {
    // This field is `Some` only when the execution status differs from the expected status.
    expected: Option<DenoSubtestStatus>,
}

async fn dump_stats(
    expected: BTreeMap<String, (u64, u64)>,
    actual: BTreeMap<String, WptMetrics>,
    output_path: &str,
) -> anyhow::Result<()> {
    let mut file = tokio::fs::File::create(output_path).await?;
    let mut total_passed = 0;
    let mut expected_total = 0;
    let mut expected_passed = 0;
    let mut lines = String::new();
    let max_width = 55;
    let default_metrics = &WptMetrics::default();

    for (suite_name, (total, passed)) in &expected {
        let key = PathBuf::from_str(&suite_name[1..])
            .expect("should parse folder into pathbuf")
            // Test suites in our own report all end in .js while those in deno's report
            // all end in .html, so we need to change the extension here to search in
            // the other map.
            .with_extension("js")
            .to_str()
            .expect("should dump folder path into str")
            .to_string();

        let metrics = actual.get(&key).unwrap_or(default_metrics);
        total_passed += metrics.passed;
        expected_total += total;
        expected_passed += passed;

        let name = if suite_name.len() > max_width {
            format!(
                "...{}",
                &suite_name[(suite_name.len() - max_width + 3)..suite_name.len()]
            )
        } else {
            suite_name.clone()
        };

        lines += &format!("|{}|{}|{}|{}|\n", name, total, passed, metrics.passed);
    }

    lines += &format!("|Total|{expected_total}|{expected_passed}|{total_passed}|\n");
    file.write_all(
        format!(
            "### WPT summary\nTotal pass rate: {:.2}%\n|Test suite|Test count|Should pass|Passed|\n|---|---|---|---|\n|Total|{}|{}|{}|\n{}",
            100f64 * total_passed as f64 / expected_total as f64,
            expected_total, expected_passed, total_passed,
            lines
        )
        .as_bytes(),
    )
    .await?;
    Ok(())
}

#[cfg_attr(feature = "skip-wpt", ignore)]
#[tokio::test]
async fn test_wpt() -> anyhow::Result<()> {
    let mut filter = TestFilter::default();
    let deno_report = DenoReport::load(
        Path::new(std::env!("CARGO_MANIFEST_DIR")).join("tests/deno_report.json"),
    )?;
    filter.set_expected_tests(deno_report.test_paths().as_slice())?;

    let report = {
        let wpt = Wpt::new().await?;
        let manifest = Wpt::read_manifest()?;
        let wpt_serve = wpt.serve(false).await?;
        WptServe::run_test_harness(&wpt_serve, &manifest, &filter, run_wpt_test).await?
    };

    let path = Path::new(std::env!("CARGO_MANIFEST_DIR")).join("tests/wptreport.json");
    let report_file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(path)
        .unwrap();
    serde_json::to_writer_pretty(report_file, &report).unwrap();

    if let Ok(v) = std::env::var("STATS_PATH") {
        dump_stats(deno_report.stats(), report.stats(), &v).await?;
    }
    Ok(())
}
