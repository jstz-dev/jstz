//! Web Platform Tests (WPT) extension for the JSTZ runtime.
//!
//! Provides test status enums, result and report structures, runtime operations and integration
//! for running WPT tests within the JSTZ kernel.

use crate::{JstzRuntime, JstzRuntimeOptions, RuntimeContext};
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
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum WptTestStatus {
    Ok = 0,
    Err = 1,
    Timeout = 2,
    PreconditionFailed = 3,
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

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct WptSubtest {
    pub name: String,
    pub status: WptSubtestStatus,
    pub message: Option<String>,
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
    pub status: Option<WptTestStatus>,
    pub subtests: Vec<WptSubtest>,
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

pub fn init_runtime(host: &mut impl HostRuntime, tx: &mut Transaction) -> JstzRuntime {
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
