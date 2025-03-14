use std::future::IntoFuture;

use anyhow::Result;
use deno_core::{
    convert::Smi,
    op2,
    v8::{self},
    FromV8, OpState, PollEventLoopOptions,
};
use deno_error::JsErrorBox;
use derive_more::{From, Into};
use expect_test::expect_file;
use jstz_core::kv::Transaction;
use jstz_crypto::{hash::Hash, smart_function_hash::SmartFunctionHash};
use jstz_runtime::JstzRuntime;
use jstz_wpt::{
    Bundle, BundleItem, TestFilter, TestToRun, Wpt, WptReportTest, WptServe, WptSubtest,
    WptSubtestStatus, WptTestStatus,
};

/// Enum of possible test statuses
///
/// More information:
///  - [wpt documentation][wpt]
///
/// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html#Test.statuses
#[derive(Debug, From, Into)]
pub struct TestStatus(WptSubtestStatus);

impl TryFrom<u8> for TestStatus {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        value.try_into().map(Self)
    }
}

/// A single subtest result
///
/// More information:
///  - [wpt documentation][wpt]
///
/// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html#Test
#[derive(Debug)]
pub struct TestResult {
    // Cannot rely on TryFromJs to convert JsString to String because TryFromJs<String> does not
    // handle utf-16 characters nicely and there are some utf-16 characters in some tests.
    // We therefore need to get JsString first and do the proper conversion.
    pub name: String,
    pub status: TestStatus,
    pub message: Option<String>,
}

impl<'a> FromV8<'a> for TestStatus {
    type Error = JsErrorBox;

    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> std::result::Result<Self, Self::Error> {
        match Smi::from_v8(scope, value) {
            Ok(Smi::<u8>(v)) => Ok(Self(
                WptSubtestStatus::try_from(v).map_err(|_| JsErrorBox::not_supported())?,
            )),
            Err(e) => Err(e),
        }
    }
}

impl<'a> FromV8<'a> for TestResult {
    type Error = JsErrorBox;

    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> std::result::Result<Self, Self::Error> {
        match value.to_object(scope) {
            Some(obj) => {
                let k0 = v8::String::new(scope, "name").unwrap();
                let k1 = v8::String::new(scope, "status").unwrap();
                let status = obj.get(scope, k1.into()).unwrap();
                let k2 = v8::String::new(scope, "message").unwrap();
                Ok(Self {
                    name: match obj.get(scope, k0.into()) {
                        Some(v) => String::from_v8(scope, v)
                            .map_err(|e| JsErrorBox::from_err(e))?,
                        None => return Err(JsErrorBox::not_supported()),
                    },
                    status: TestStatus::from_v8(scope, status)?,
                    message: match obj.get(scope, k2.into()) {
                        Some(v) => Some(
                            String::from_v8(scope, v)
                                .map_err(|e| JsErrorBox::from_err(e))?,
                        ),
                        None => None,
                    },
                })
            }
            None => Err(JsErrorBox::not_supported()),
        }
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

impl TryFrom<u8> for TestsStatus {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        value.try_into().map(Self)
    }
}

impl<'a> FromV8<'a> for TestsStatus {
    type Error = JsErrorBox;

    fn from_v8(
        scope: &mut v8::HandleScope<'a>,
        value: v8::Local<'a, v8::Value>,
    ) -> std::result::Result<Self, Self::Error> {
        match Smi::from_v8(scope, value) {
            Ok(Smi::<u8>(v)) => Ok(Self(
                WptTestStatus::try_from(v).map_err(|_| JsErrorBox::not_supported())?,
            )),
            Err(e) => Err(e),
        }
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
    ) -> std::result::Result<Self, Self::Error> {
        match value.to_object(scope) {
            Some(obj) => {
                let k1 = v8::String::new(scope, "status").unwrap();
                let status = obj.get(scope, k1.into()).unwrap();
                let k2 = v8::String::new(scope, "message").unwrap();
                Ok(Self {
                    status: TestsStatus::from_v8(scope, status)?,
                    message: match obj.get(scope, k2.into()) {
                        Some(v) => Some(
                            String::from_v8(scope, v)
                                .map_err(|e| JsErrorBox::from_err(e))?,
                        ),
                        None => None,
                    },
                })
            }
            None => Err(JsErrorBox::not_supported()),
        }
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
    pub fn set_harness_result(&mut self, result: TestsResult) -> Result<()> {
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
            // `to_std_string_escaped` handles utf-16 characters better than `to_std_string`
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
) -> core::result::Result<(), JsErrorBox> {
    // The first argument (0) is an array of `Test` objects [ignored]
    // The second argument (1) is the `TestsStatus` object
    // The final argument (2) is an array of `AssertRecord` objects [ignored]
    let report: &mut TestHarnessReport = op_state.borrow_mut::<TestHarnessReport>();
    report
        .set_harness_result(result)
        .map_err(|e| JsErrorBox::generic(e.to_string()))
}

async fn register_callback(rt: &mut JstzRuntime) {
    rt.execute_with_result::<()>("add_result_callback(globalThis.test_completion_callback); add_completion_callback(globalThis.test_completion_callback);");
}

deno_core::extension!(
    test_harness_api,
    ops = [test_completion_callback, test_result_callback],
    esm_entry_point = "ext:tests/test_harness_api.js",
    esm = [dir "tests", "test_harness_api.js"],
);

pub async fn run_wpt_test_harness(bundle: &Bundle) -> Result<TestHarnessReport> {
    let address =
        SmartFunctionHash::from_base58("KT1RJ6PbjHpwc3M5rw5s2Nbmefwbuwbdxton").unwrap();
    let tx = &mut Transaction::default();
    tx.begin();
    let mut host = tezos_smart_rollup_mock::MockHost::default();
    let mut rt = JstzRuntime::new(&mut host, tx, address);

    //insert_global_properties(&mut rt);

    // Run the bundle, evaluating each script in order
    // Instead of loading the TestHarnessReport script, we initialize it manually
    for item in &bundle.items {
        match item {
            BundleItem::TestHarnessReport => {
                register_callback(&mut rt).await;
            }
            BundleItem::Inline(script) | BundleItem::Resource(_, script) => {
                let _: Option<_> = rt.execute_with_result::<()>(script);
            }
        }
    }

    // Execute promises after all sync tests have completed after `eval` returns
    rt.run_event_loop(PollEventLoopOptions::default()).await?;

    // Return the test harness report
    let data: TestHarnessReport =
        rt.op_state().borrow().borrow::<TestHarnessReport>().clone();

    Ok(data)
}

fn run_wpt_test(
    wpt_serve: &WptServe,
    test: TestToRun,
) -> impl IntoFuture<Output = Result<WptReportTest>> + '_ {
    async move {
        let bundle = wpt_serve.bundle(&test.url_path).await?;

        let Ok(report) = run_wpt_test_harness(&bundle).await else {
            return Ok(WptReportTest::new(WptTestStatus::Err, vec![]));
        };

        // It should be safe to unwrap here because each test suite should have a
        // status code attached after it completes. If unwrap fails, it means something
        // is wrong and we should fix that
        let status = report.status.clone().unwrap();

        let subtests = report.subtests.clone();

        Ok(WptReportTest::new(status, subtests))
    }
}

#[cfg_attr(feature = "skip-wpt", ignore)]
#[tokio::test]
async fn test_wpt() -> Result<()> {
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
            r"^\/url\/[^\/]+\.any\.html$", // URL, URLSearchParams
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
        ]
        .as_ref(),
    )?;

    let report = {
        let wpt = Wpt::new().await?;

        let manifest = Wpt::read_manifest()?;

        let wpt_serve = wpt.serve(false).await?;

        WptServe::run_test_harness(&wpt_serve, &manifest, &filter, run_wpt_test).await?
    };

    let expected = expect_file!["./wptreport.json"];

    expected.assert_eq(&serde_json::to_string_pretty(&report)?);

    Ok(())
}
