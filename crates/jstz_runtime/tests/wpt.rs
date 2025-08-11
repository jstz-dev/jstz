use anyhow::Context;
use deno_core::{
    convert::Smi,
    op2,
    v8::{self},
    FromV8, OpState,
};
use deno_error::JsErrorBox;
use derive_more::{From, Into};

use jstz_core::{host::HostRuntime, kv::Transaction};
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use jstz_runtime::{JstzRuntime, JstzRuntimeOptions, RuntimeContext};
use jstz_wpt::{
    Bundle, BundleItem, TestFilter, TestToRun, Wpt, WptMetrics, WptReportTest, WptServe,
    WptSubtest, WptSubtestStatus, WptTestStatus,
};
use regex::Regex;
use ron::de::from_str as ron_from_str;
use serde::Deserialize;
use std::borrow::Cow;
use std::{
    collections::{BTreeMap, HashSet},
    fs::{File, OpenOptions},
    future::IntoFuture,
    panic,
    path::{Path, PathBuf},
    str::FromStr,
};
use tezos_smart_rollup_mock::MockHost;
use thiserror::Error;
use tokio::io::AsyncWriteExt;

/// List of tests that should be skipped due to known issues (e.g., segmentation faults)
/// Add test paths here as you encounter problematic tests
const SKIP_TESTS: &[&str] = &[
    // Add problematic tests here, for example:
    "FileAPI/url/url-format.any.html",
    "compression/compression",
    "encoding/", // "some/other/problematic/test.html",
    "fetch/http-cache/",
    "webstorage/",
    "html/webappapis/scripting/processing-model-2",
];

/// Check if a test should be skipped
fn should_skip_test(test_path: &str) -> bool {
    SKIP_TESTS
        .iter()
        .any(|&skip_path| test_path.contains(skip_path))
}

/// Enum of possible test statuses
///
/// More information:
///  - [wpt documentation][wpt]
///
/// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html#Test.statuses
#[derive(Debug, From, Into)]
pub struct TestStatus(WptSubtestStatus);

/// A single subtest result
///
/// More information:
///  - [wpt documentation][wpt]
///
/// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html#Test
#[derive(Debug)]
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub message: Option<String>,
}

impl<'a> FromV8<'a> for TestStatus {
    type Error = JsErrorBox;

    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> Result<Self, Self::Error> {
        Smi::<u8>::from_v8(scope, value)?
            .0
            .try_into()
            .map(Self)
            .map_err(|_| {
                let s = value.to_rust_string_lossy(scope);
                JsErrorBox::generic(format!(
                    "failed to convert value '{s}' into TestStatus",
                ))
            })
    }
}

impl<'a> FromV8<'a> for TestResult {
    type Error = JsErrorBox;

    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> Result<Self, Self::Error> {
        let obj = value
            .to_object(scope)
            .ok_or(JsErrorBox::generic("TestResult must be a JS object"))?;

        let name_key = v8::String::new(scope, "name").unwrap();
        let local_name = obj.get(scope, name_key.into()).ok_or(JsErrorBox::generic(
            "property 'name' must be present in TestResult",
        ))?;
        let status_key = v8::String::new(scope, "status").unwrap();
        let local_status =
            obj.get(scope, status_key.into())
                .ok_or(JsErrorBox::generic(
                    "property 'status' must be present in TestResult",
                ))?;
        let message_key = v8::String::new(scope, "message").unwrap();
        let message = match obj.get(scope, message_key.into()) {
            Some(v) => Some(String::from_v8(scope, v).map_err(JsErrorBox::from_err)?),
            None => None,
        };

        Ok(Self {
            name: String::from_v8(scope, local_name).map_err(JsErrorBox::from_err)?,
            status: TestStatus::from_v8(scope, local_status)?,
            message,
        })
    }
}

/// Enum of possible harness statuses
///
/// More information:
///  - [wpt documentation][wpt]
///
/// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html#TestsStatus.statuses
#[derive(Debug, From, Into)]
pub struct TestsStatus(WptTestStatus);

impl<'a> FromV8<'a> for TestsStatus {
    type Error = JsErrorBox;

    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> Result<Self, Self::Error> {
        Smi::<u8>::from_v8(scope, value)?
            .0
            .try_into()
            .map(Self)
            .map_err(|_| {
                let s = value.to_rust_string_lossy(scope);
                JsErrorBox::generic(format!(
                    "failed to convert value '{s}' into TestsStatus",
                ))
            })
    }
}

/// The result of a test harness run
///
/// More information:
///  - [wpt documentation][wpt]
///
/// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html#TestsStatus.statuses
pub struct TestsResult {
    pub status: TestsStatus,
    pub message: Option<String>,
}

impl<'a> FromV8<'a> for TestsResult {
    type Error = JsErrorBox;

    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> Result<Self, Self::Error> {
        let obj = value
            .to_object(scope)
            .ok_or(JsErrorBox::generic("TestsResult must be a JS object"))?;

        let status_key = v8::String::new(scope, "status").unwrap();
        let local_status =
            obj.get(scope, status_key.into())
                .ok_or(JsErrorBox::generic(
                    "property 'status' must be present in TestsResult",
                ))?;
        let message_key = v8::String::new(scope, "message").unwrap();
        let message = match obj.get(scope, message_key.into()) {
            Some(v) => Some(String::from_v8(scope, v).map_err(JsErrorBox::from_err)?),
            None => None,
        };

        Ok(Self {
            status: TestsStatus::from_v8(scope, local_status)?,
            message,
        })
    }
}

/// A report of a test harness run, containing the harness result and all test results
///
/// This struct implements the TestHarness API expected by [wpt]
///
/// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html
#[derive(Default, Debug, Clone, Deserialize)]
pub struct TestHarnessReport {
    // `status` is an Option because it is set at the end of a test suite
    // and we need a placeholder for it before that.
    status: Option<WptTestStatus>,
    subtests: Vec<WptSubtest>,
}

impl TestHarnessReport {
    /// Sets the harness result, if it has not already been set
    ///
    /// # Errors
    ///
    /// Returns an error if the harness result has already been set
    pub fn set_harness_result(&mut self, result: TestsResult) -> anyhow::Result<()> {
        if self.status.is_some() {
            anyhow::bail!("Harness result already set");
        }

        self.status = Some(result.status.into());
        Ok(())
    }

    /// Adds a test result to the report
    pub fn add_test_result(&mut self, result: TestResult) {
        let TestResult {
            name,
            status,
            message,
        } = result;

        self.subtests.push(WptSubtest {
            name,
            status: status.into(),
            message,
        });
    }
}
#[derive(Deserialize)]
struct LogLine<'a> {
    message: Cow<'a, str>,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("log line contained a report, but it couldn't be parsed as RON: {0}")]
    Ron(String),
}

/// Try to get a TestHarnessReport out of *any* line of text.
/// - Returns Ok(Some(...)) if a report was found and parsed
/// - Returns Ok(None) if no report-like text exists on the line
/// - Returns Err(_) only if a report-like text was found but failed to parse
pub fn parse_report_from_log_line(
    line: &str,
) -> Result<Option<TestHarnessReport>, ParseError> {
    // 1) Try to slice a report straight from the raw line (works for arbitrary/non-JSON logs)
    if let Some(raw_dbg) = slice_report_debug(line) {
        return parse_report_debug_text(raw_dbg).map(Some);
    }

    // 2) Fallback: if the line *is* JSON with a "message" field, try inside it
    if let Ok(env) = serde_json::from_str::<LogLine>(line) {
        if let Some(raw_dbg) = slice_report_debug(&env.message) {
            return parse_report_debug_text(raw_dbg).map(Some);
        }
    }

    Ok(None)
}

/// Parse the Rust Debug-ish "TestHarnessReport { ... }" into a struct,
/// handling both raw text and JSON-escaped text (`\"`, `\\n`, etc).
fn parse_report_debug_text(raw_dbg: &str) -> Result<TestHarnessReport, ParseError> {
    // If this came from inside a JSON string, its quotes are escaped as \".
    // Try RON directly first; if that fails, try JSON-unescaping the snippet and parse again.
    match try_ron_from_debug(raw_dbg) {
        Ok(rep) => Ok(rep),
        Err(_) => {
            if let Some(unescaped) = try_unescape_as_json_string(raw_dbg) {
                try_ron_from_debug(&unescaped).map_err(|e| ParseError::Ron(e))
            } else {
                Err(ParseError::Ron(
                    "RON parse failed and JSON-unescape also failed".into(),
                ))
            }
        }
    }
}

/// Convert the Debug text to RON and attempt to parse.
/// Returns Err(stringified error) on failure.
fn try_ron_from_debug(debug_text: &str) -> Result<TestHarnessReport, String> {
    let ron_text = braces_to_parens_preserving_strings(debug_text);
    ron_from_str::<TestHarnessReport>(&ron_text).map_err(|e| e.to_string())
}

/// If `raw` looks like it came from inside a JSON string (e.g., it contains `\"`),
/// try to interpret it as a JSON string and unescape it.
fn try_unescape_as_json_string(raw: &str) -> Option<String> {
    // Only attempt if there are typical JSON escapes. This avoids breaking truly-raw inputs.
    if !(raw.contains("\\\"")
        || raw.contains("\\n")
        || raw.contains("\\t")
        || raw.contains("\\r")
        || raw.contains("\\\\"))
    {
        return None;
    }
    // Wrap in quotes and let serde_json do the heavy lifting.
    let wrapped = format!("\"{}\"", raw);
    serde_json::from_str::<String>(&wrapped).ok()
}

/// Extracts "TestHarnessReport { ... }" from any text.
/// If "Internal message:" follows, we stop before it; otherwise we bracket-match to the closing '}'.
fn slice_report_debug(s: &str) -> Option<&str> {
    let start = s.find("TestHarnessReport")?;
    let after = &s[start..];

    // If an explicit sentinel exists, crop to it first to avoid scanning the whole line.
    if let Some(end_sentinel) = after.find("Internal message:") {
        let candidate = &after[..end_sentinel];
        // Verify it looks like a report (has an opening brace after the type name).
        if candidate.find('{').is_some() {
            return Some(candidate.trim());
        }
    }

    // Otherwise, bracket-match starting at the first '{' after the type name.
    let open_rel = after.find('{')?;
    let open_idx = start + open_rel;

    let mut depth: i32 = 0;
    let mut in_str = false;
    let mut prev_bs = false;
    let mut end_idx = None;

    for (i, ch) in s[open_idx..].char_indices() {
        if in_str {
            match ch {
                '\\' => prev_bs = !prev_bs,
                '"' if !prev_bs => {
                    in_str = false;
                    prev_bs = false;
                }
                _ => prev_bs = false,
            }
            continue;
        }

        match ch {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end_idx = Some(open_idx + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }

    end_idx.map(|end| s[start..end].trim())
}

/// Replace `{`/`}` with `(`/`)` outside of quoted strings.
/// This turns a Debug-ish struct into RON-compatible text.
fn braces_to_parens_preserving_strings(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_str = false;
    let mut prev_bs = false;

    for ch in input.chars() {
        if in_str {
            out.push(ch);
            match ch {
                '\\' => prev_bs = !prev_bs,
                '"' if !prev_bs => {
                    in_str = false;
                    prev_bs = false;
                }
                _ => prev_bs = false,
            }
        } else {
            match ch {
                '"' => {
                    in_str = true;
                    out.push(ch);
                }
                '{' => out.push('('),
                '}' => out.push(')'),
                _ => out.push(ch),
            }
        }
    }

    out
}

#[op2]
pub fn test_result_callback(op_state: &mut OpState, #[from_v8] result: TestResult) {
    let report: &mut TestHarnessReport = op_state.borrow_mut::<TestHarnessReport>();
    report.add_test_result(result);
}

#[op2]
pub fn test_completion_callback(
    op_state: &mut OpState,
    _tests: &v8::Value,
    #[from_v8] result: TestsResult,
    _records: &v8::Value,
) -> Result<(), JsErrorBox> {
    let report: &mut TestHarnessReport = op_state.borrow_mut::<TestHarnessReport>();
    report
        .set_harness_result(result)
        .map_err(|e| JsErrorBox::generic(e.to_string()))
}

deno_core::extension!(
    test_harness_api,
    ops = [test_completion_callback, test_result_callback],
    esm_entry_point = "ext:test_harness_api/test_harness_api.js",
    esm = [dir "tests", "test_harness_api.js"],
);

fn init_runtime(host: &mut impl HostRuntime, tx: &mut Transaction) -> JstzRuntime {
    let address =
        SmartFunctionHash::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton").unwrap();

    let mut options = JstzRuntimeOptions::default();
    options
        .extensions
        .push(test_harness_api::init_ops_and_esm());

    let mut runtime = JstzRuntime::new(JstzRuntimeOptions {
        protocol: Some(RuntimeContext::new(host, tx, address, String::new())),
        extensions: vec![test_harness_api::init_ops_and_esm()],
        ..Default::default()
    });

    let op_state = runtime.op_state();
    // Insert a blank report to be filled in by test cases
    op_state.borrow_mut().put(TestHarnessReport::default());

    runtime
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

pub async fn run_wpt_tests() -> anyhow::Result<()> {
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

#[cfg_attr(feature = "skip-wpt", ignore)]
#[tokio::test]
async fn test_wpt() -> anyhow::Result<()> {
    run_wpt_tests().await?;
    Ok(())
}
