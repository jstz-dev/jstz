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
use jstz_runtime::{JstzRuntime, JstzRuntimeOptions, Protocol};
use jstz_wpt::{
    Bundle, BundleItem, TestFilter, TestToRun, Wpt, WptMetrics, WptReport, WptReportTest,
    WptServe, WptSubtest, WptSubtestStatus, WptTestStatus,
};
use std::{fs::OpenOptions, future::IntoFuture, path::Path};
use tezos_smart_rollup_mock::MockHost;
use tokio::io::AsyncWriteExt;

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
#[derive(Default, Debug, Clone)]
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

    let mut options = JstzRuntime::options();
    options
        .extensions
        .push(test_harness_api::init_ops_and_esm());

    let mut runtime = JstzRuntime::new(JstzRuntimeOptions {
        protocol: Some(Protocol::new(host, tx, address)),
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

    let mut rt = init_runtime(&mut host, &mut tx);

    // Somehow each `execute_script` call has some strange side effect such that the global
    // test suite object is completed prematurely before all test cases are registered.
    // Therefore, instead of executing each piece of test scripts separately, we need to
    // collect them and run them all in one `execute_script` call.
    let _ = rt.execute_script("native code", source);

    // Take the test harness report out of the runtime and return it
    // Need to store data temporarily so that the borrow can be dropped
    let data = rt.op_state().borrow().borrow::<TestHarnessReport>().clone();
    data
}

fn run_wpt_test(
    wpt_serve: &WptServe,
    test: TestToRun,
) -> impl IntoFuture<Output = anyhow::Result<WptReportTest>> + '_ {
    async move {
        let bundle = wpt_serve.bundle(&test.url_path).await?;
        let report = run_wpt_test_harness(&bundle).await;
        println!("Running test {} => {:?}", &test.url_path, &report.status);
        // Each test suite should have a status code attached after it completes.
        // When unwrap fails, it means something is wrong, e.g. some tests failed because
        // of something not yet supported by the runtime, such that the test completion callback
        // was not even triggered and we should fix that.
        let status = report.status.clone().unwrap_or(WptTestStatus::Err);
        let subtests = report.subtests.clone();
        Ok(WptReportTest::new(status, subtests))
    }
}

async fn dump_stats(report: &WptReport, output_path: &str) -> anyhow::Result<()> {
    let mut file = tokio::fs::File::create(output_path).await?;
    let mut full_metrics = WptMetrics::default();
    let mut lines =
        "|Test suite|Passed|Failed|Timed out|\n|---|---|---|---|\n".to_string();

    let max_width = 45; // line width is around 80
    for (suite_name, metrics) in report.stats() {
        full_metrics.passed += metrics.passed;
        full_metrics.failed += metrics.failed;
        full_metrics.timed_out += metrics.timed_out;
        let name = if suite_name.len() > max_width {
            format!(
                "...{}",
                &suite_name[(suite_name.len() - max_width + 3)..suite_name.len()]
            )
        } else {
            suite_name
        };
        lines += &format!(
            "|{}|{}|{}|{}|\n",
            name, metrics.passed, metrics.failed, metrics.timed_out
        );
    }

    lines += &format!(
        "|Total|{}|{}|{}|\n",
        full_metrics.passed, full_metrics.failed, full_metrics.timed_out
    );
    file.write_all(
        format!(
            "### WPT summary\nTotal pass rate: {:.2}%\n{}",
            100f64 * full_metrics.passed as f64
                / (full_metrics.passed + full_metrics.failed + full_metrics.timed_out)
                    as f64,
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
    let filter = TestFilter::try_from(
        [
            r"^\/encoding\/[^\/]+\.any\.html$", // TextEncode, TextDecoder
            r"^\/encoding\/streams\/[^\/]+\.any\.html$", // TextEncoderStream, TextDecoderStream
            r"^\/fetch\/api\/headers\/[^\/]+\.any\.html$",
            r"^\/FileAPI\/blob\/[^\/]+\.any\.html$", // Blob
            r"^\/streams\/queuing\-strategies\.any\.html$", // CountQueuingStrategy, ByteLengthQueuingStrategy
            // WritableStream, WritableStreamDefaultController, ByteLengthQueuingStrategy, CountQueuingStrategy
            r"^\/streams\/writable\-streams\/.+\.any\.html$",
            r"^\/compression\/[^\/]+\.any\.html$", // CompressionStream, DecompressionStream
            // module crypto; tests have "Err" status now because `crypto` does not exist in global yet
            r"^\/WebCryptoAPI\/.+\.any\.html$",
            r"^\/streams\/readable\-streams\/.+\.any\.html$", // ReadableStream
            // ReadableByteStreamController
            // construct-byob-request.any.js shows Err because `ReadableStream` and `ReadableByteStreamController`
            // are not yet implemented
            r"^\/streams\/readable\-byte\-streams\/.+\.any\.html$",
            r"^\/streams\/transform\-streams\/.+\.any\.html$", // TransformStream
            r"^\/url\/[^\/]+\.any\.html$",                     // URL, URLSearchParams
            // Request
            // request-structure.any.js shows Err because jstz Request does not accept empty URLs
            r"^\/fetch\/api\/request\/[^\/]+\.any\.html$",
            // Response
            // FIXME: after JSTZ-328 is fixed, update the following lines so that all
            // `/fetch/api/response` test suites are enabled. The test suite being filtered out is
            // `fetch/api/response/response-static-json.any.js`
            r"^\/fetch\/api\/response\/response-[^s].+\.any\.html$",
            r"^\/fetch\/api\/response\/response-static-[^j].+\.any\.html$",
            r"^\/fetch\/api\/response\/response-stream-.+\.any\.html$",
            r"^\/html\/webappapis\/atob\/base64\.any\.html$", // atob, btoa
            r"^\/html\/webappapis\/structured-clone\/structured\-clone\.any\.html$", // structuredClone
            // set/clearTimeout, set/clearInterval
            // Some tests show Err because the targeted set/clear functions are not yet defined
            r"^\/html\/webappapis\/timers\/[^\/]+\.any\.html$",
            r"^\/xhr\/formdata\/[^\/]+\.any\.html$", // FormData
            r"^\/console\/[^\/]+\.any\.html$",       // console
        ]
        .as_ref(),
    )?;

    let report = {
        let wpt = Wpt::new().await?;
        let manifest = Wpt::read_manifest()?;
        let wpt_serve = wpt.serve(false).await?;
        WptServe::run_test_harness(&wpt_serve, &manifest, &filter, run_wpt_test).await?
    };
    let path = Path::new(std::env!("CARGO_MANIFEST_DIR")).join("tests/wptreport.json");
    if std::env::var("REGEN").is_ok() {
        let report_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(path)
            .unwrap();
        serde_json::to_writer_pretty(report_file, &report).unwrap();
    } else {
        let expected_file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(expected_file);
        let expected: WptReport = serde_json::from_reader(reader).unwrap();
        assert_eq!(expected, report);
    }
    if let Ok(v) = std::env::var("STATS_PATH") {
        dump_stats(&report, &v).await?;
    }
    Ok(())
}
