use std::future::IntoFuture;

use anyhow::Result;
use boa_engine::{
    js_string, object::FunctionObjectBuilder, property::PropertyDescriptor,
    value::TryFromJs, Context, JsArgs, JsNativeError, JsResult, JsValue, NativeFunction,
    Source,
};
use boa_gc::{Finalize, Trace};
use derive_more::{From, Into};
use expect_test::expect_file;
use jstz_core::{host_defined, Api, Runtime};
use jstz_wpt::{
    Bundle, BundleItem, TestFilter, TestToRun, Wpt, WptReportTest, WptServe, WptSubtest,
    WptSubtestStatus, WptTestStatus,
};

macro_rules! impl_try_from_js_for_enum {
    ($ty:ty) => {
        impl TryFromJs for $ty {
            fn try_from_js(value: &JsValue, context: &mut Context<'_>) -> JsResult<Self> {
                let value: u8 = value.try_js_into(context)?;

                value.try_into().map_err(|_| {
                    JsNativeError::eval()
                        .with_message(format!("Invalid ${}", stringify!($ty)))
                        .into()
                })
            }
        }
    };
}

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

impl_try_from_js_for_enum!(TestStatus);

/// A single subtest result
///
/// More information:
///  - [wpt documentation][wpt]
///
/// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html#Test
#[derive(Debug, TryFromJs)]
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub message: Option<String>,
}

/// Enum of possible harness statuses
///
/// More information:
///  - [wpt documentation][wpt]
///
/// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html#TestsStatus.statuses
#[derive(Debug, From, Into)]
pub struct TestsStatus(WptTestStatus);

impl_try_from_js_for_enum!(TestsStatus);

impl TryFrom<u8> for TestsStatus {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        value.try_into().map(Self)
    }
}

/// The result of a test harness run
///
/// More information:
///  - [wpt documentation][wpt]
///
/// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html#TestsStatus.statuses
#[derive(TryFromJs)]
pub struct TestsResult {
    pub status: TestsStatus,
    pub message: Option<String>,
}

/// A report of a test harness run, containing the harness result and all test results
///
/// This struct implements the TestHarness API expected by [wpt]
///
/// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html
#[derive(Default, Debug, Trace, Finalize)]
pub struct TestHarnessReport {
    #[unsafe_ignore_trace]
    status: Option<WptTestStatus>,
    #[unsafe_ignore_trace]
    subtests: Vec<WptSubtest>,
}

impl TestHarnessReport {
    /// Sets the harness result, if it has not already been set
    ///
    /// # Errors
    ///
    /// Returns an error if the harness result has already been set
    pub fn set_harness_result(&mut self, result: TestsResult) -> JsResult<()> {
        if self.status.is_some() {
            return Err(JsNativeError::eval()
                .with_message("Harness result already set")
                .into());
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

/// The test harness report jstz API (bound to the global object)
pub struct TestHarnessReportApi;

macro_rules! preamble {
    ($context:expr, $report:ident) => {
        host_defined!($context, mut host_defined);
        let mut $report = host_defined
            .get_mut::<TestHarnessReport>()
            .expect("TestHarnessReport undefined");
    };
}

impl TestHarnessReportApi {
    /// The add_result_callback function for jstz's test harness reports
    ///
    /// More information:
    ///  - [wpt documentation][wpt]
    ///
    /// [wpt]: https://web-platform-tests.org/writing-tests/testharness-api.html#add_result_callback
    pub fn test_result_callback(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        preamble!(context, report);

        let result: TestResult = args.get_or_undefined(0).try_js_into(context)?;

        report.add_test_result(result);

        Ok(JsValue::undefined())
    }

    pub fn test_completion_callback(
        _this: &JsValue,
        args: &[JsValue],
        context: &mut Context<'_>,
    ) -> JsResult<JsValue> {
        preamble!(context, report);

        // The first argument (0) is an array of `Test` objects [ignored]
        // The second argument (1) is the `TestsStatus` object
        // The final argument (2) is an array of `AssertRecord` objects [ignored]
        let result: TestsResult = args.get_or_undefined(1).try_js_into(context)?;

        report.set_harness_result(result)?;

        Ok(JsValue::undefined())
    }
}

impl jstz_core::Api for TestHarnessReportApi {
    fn init(self, context: &mut Context<'_>) {
        let test_result_callback = FunctionObjectBuilder::new(
            context.realm(),
            NativeFunction::from_fn_ptr(Self::test_result_callback),
        )
        .name("test_result_callback")
        .length(1)
        .build();

        let test_completion_callback = FunctionObjectBuilder::new(
            context.realm(),
            NativeFunction::from_fn_ptr(Self::test_completion_callback),
        )
        .name("test_completion_callback")
        .length(3)
        .build();

        #[inline]
        fn call_global_function(name: &str, args: &[JsValue], context: &mut Context<'_>) {
            let value = context
                .global_object()
                .get(js_string!(name), context)
                .expect(&format!("globalThis.{} is undefined", name));

            let function = value
                .as_callable()
                .expect(&format!("globalThis.{} is not callable", name));

            function
                .call(&JsValue::undefined(), args, context)
                .expect(&format!("Failed to call globalThis.{}", name));
        }

        call_global_function(
            "add_result_callback",
            &[test_result_callback.into()],
            context,
        );
        call_global_function(
            "add_completion_callback",
            &[test_completion_callback.into()],
            context,
        );
    }
}

pub fn register_apis(context: &mut Context<'_>) {
    // Register all the APIs here
    // TODO this is not all the APIs
    jstz_api::http::header::HeadersApi.init(context);
    jstz_api::encoding::EncodingApi.init(context);
}

pub fn run_wpt_test_harness(bundle: &Bundle) -> JsResult<Box<TestHarnessReport>> {
    let mut rt: Runtime<'_> = Runtime::new(usize::MAX)?;

    // Initialize the host-defined object with the test harness report
    {
        host_defined!(&mut rt, mut host_defined);
        host_defined.insert(TestHarnessReport::default());
    }

    // Register APIs
    register_apis(&mut rt);

    // Define self
    rt.global_object().insert_property(
        js_string!("self"),
        PropertyDescriptor::builder()
            .value(rt.global_object().clone())
            .configurable(true)
            .writable(true)
            .enumerable(true)
            .build(),
    );

    // Run the bundle, evaluating each script in order
    // Instead of loading the TestHarnessReport script, we initialize it manually
    for item in &bundle.items {
        match item {
            BundleItem::TestHarnessReport => {
                TestHarnessReportApi.init(rt.context());
            }
            BundleItem::Inline(script) | BundleItem::Resource(_, script) => {
                rt.context().eval(Source::from_bytes(script))?;
            }
        }
    }

    // Return the test harness report

    let test_harness_report = {
        host_defined!(&mut rt, mut host_defined);
        host_defined
            .remove::<TestHarnessReport>()
            .expect("TestHarnessReport undefined")
    };

    Ok(test_harness_report)
}

fn run_wpt_test(
    wpt_serve: &WptServe,
    test: TestToRun,
) -> impl IntoFuture<Output = Result<WptReportTest>> + '_ {
    async move {
        let bundle = wpt_serve.bundle(&test.url_path).await?;

        let Ok(report) = run_wpt_test_harness(&bundle) else {
            return Ok(WptReportTest::new(WptTestStatus::Err, vec![]));
        };

        let status = report.status.clone().unwrap_or(WptTestStatus::Null);

        let subtests = report.subtests.clone();

        Ok(WptReportTest::new(status, subtests))
    }
}

#[tokio::test]
async fn test_wpt() -> Result<()> {
    let filter = TestFilter::try_from(
        [
            r"^\/encoding\/[^\/]+\.any\.html$",
            r"^\/fetch\/api\/headers\/[^\/]+\.any\.html$",
        ]
        .as_ref(),
    )?;

    let report = {
        let wpt = Wpt::new()?;

        let manifest = Wpt::read_manifest()?;

        let wpt_serve = wpt.serve(false).await?;

        WptServe::run_test_harness(&wpt_serve, &manifest, &filter, run_wpt_test).await?
    };

    let expected = expect_file!["./wptreport.json"];

    expected.assert_eq(&serde_json::to_string_pretty(&report)?);

    Ok(())
}
