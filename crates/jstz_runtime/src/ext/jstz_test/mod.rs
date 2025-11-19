// Copyright 2018-2025 the Deno authors. MIT license.

use std::fmt;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use deno_core::{error::JsError, extension, op2, v8, ModuleSpecifier, OpState};
use deno_error::JsErrorBox;
use indexmap::{IndexMap, IndexSet};
use serde::Deserialize;
use tokio::sync::mpsc::{error::SendError, UnboundedReceiver, UnboundedSender};

pub mod executor;

#[derive(Debug, Clone, PartialEq, Deserialize, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TestLocation {
    pub file_name: String,
    pub line_number: u32,
    pub column_number: u32,
}

#[derive(Default)]
pub(crate) struct TestContainer {
    descriptions: TestDescriptions,
    test_functions: Vec<v8::Global<v8::Function>>,
    test_hooks: TestHooks,
}

#[derive(Default)]
pub(crate) struct TestHooks {
    pub before_all: Vec<v8::Global<v8::Function>>,
    pub before_each: Vec<v8::Global<v8::Function>>,
    pub after_each: Vec<v8::Global<v8::Function>>,
    pub after_all: Vec<v8::Global<v8::Function>>,
}

impl TestContainer {
    pub fn register(
        &mut self,
        description: TestDescription,
        function: v8::Global<v8::Function>,
    ) {
        self.descriptions.tests.insert(description.id, description);
        self.test_functions.push(function)
    }

    pub fn register_hook(
        &mut self,
        hook_type: String,
        function: v8::Global<v8::Function>,
    ) {
        match hook_type.as_str() {
            "beforeAll" => self.test_hooks.before_all.push(function),
            "beforeEach" => self.test_hooks.before_each.push(function),
            "afterEach" => self.test_hooks.after_each.push(function),
            "afterAll" => self.test_hooks.after_all.push(function),
            _ => {}
        }
    }
}

#[derive(Default, Debug)]
pub struct TestDescriptions {
    tests: IndexMap<usize, TestDescription>,
}

impl TestDescriptions {
    pub fn register(&mut self, description: TestDescription) {
        self.tests.insert(description.id, description);
    }

    pub fn len(&self) -> usize {
        self.tests.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tests.is_empty()
    }
}

impl<'a> IntoIterator for &'a TestDescriptions {
    type Item = <&'a IndexMap<usize, TestDescription> as IntoIterator>::Item;
    type IntoIter = <&'a IndexMap<usize, TestDescription> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        (&self.tests).into_iter()
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TestDescription {
    pub id: usize,
    pub name: String,
    pub ignore: bool,
    pub only: bool,
    pub origin: String,
    pub location: TestLocation,
    pub sanitize_ops: bool,
    pub sanitize_resources: bool,
}

/// May represent a failure of a test or test step.
#[derive(Debug, Clone, PartialEq, Deserialize, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TestFailureDescription {
    pub id: usize,
    pub name: String,
    pub origin: String,
    pub location: TestLocation,
}

impl From<&TestDescription> for TestFailureDescription {
    fn from(value: &TestDescription) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
            origin: value.origin.clone(),
            location: value.location.clone(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct TestFailureFormatOptions {
    pub hide_stacktraces: bool,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestFailure {
    JsError(Box<JsError>),
    FailedSteps(usize),
    IncompleteSteps,
    Leaked(Vec<String>, Vec<String>), // Details, trailer notes
    // The rest are for steps only.
    Incomplete,
    OverlapsWithSanitizers(IndexSet<String>), // Long names of overlapped tests
    HasSanitizersAndOverlaps(IndexSet<String>), // Long names of overlapped tests
}

impl TestFailure {
    pub fn error_location(&self) -> Option<TestLocation> {
        let TestFailure::JsError(js_error) = self else {
            return None;
        };
        // The first line of user code comes above the test file.
        // The call stack usually contains the top 10 frames, and cuts off after that.
        // We need to explicitly check for the test runner here.
        // - Checking for a `ext:` is not enough, since other Deno `ext:`s can appear in the call stack.
        // - This check guarantees that the next frame is inside of the Deno.test(),
        //   and not somewhere else.
        const TEST_RUNNER: &str = "ext:cli/40_test.js";
        let runner_frame_index = js_error
            .frames
            .iter()
            .position(|f| f.file_name.as_deref() == Some(TEST_RUNNER))?;
        let frame = js_error
            .frames
            .split_at(runner_frame_index)
            .0
            .iter()
            .rfind(|f| {
                f.file_name.as_ref().is_some_and(|f| {
                    f.starts_with("file:") && !f.contains("node_modules")
                })
            })?;
        let file_name = frame.file_name.as_ref()?.clone();
        // Turn into zero based indices
        let line_number = frame.line_number.map(|v| v - 1)? as u32;
        let column_number = frame.column_number.map(|v| v - 1).unwrap_or(0) as u32;
        Some(TestLocation {
            file_name,
            line_number,
            column_number,
        })
    }

    pub fn hide_in_summary(&self) -> bool {
        // These failure variants are hidden in summaries because they are caused
        // by child errors that will be summarized separately.
        matches!(
            self,
            TestFailure::FailedSteps(_) | TestFailure::IncompleteSteps
        )
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestResult {
    Ok,
    Ignored,
    Failed(TestFailure),
    Cancelled,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestStepDescription {
    pub id: usize,
    pub name: String,
    pub origin: String,
    pub location: TestLocation,
    pub level: usize,
    pub parent_id: usize,
    pub root_id: usize,
    pub root_name: String,
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestStepResult {
    Ok,
    Ignored,
    Failed(TestFailure),
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestPlan {
    pub origin: String,
    pub total: usize,
    pub filtered_out: usize,
    pub used_only: bool,
}

// TODO(bartlomieju): in Rust 1.90 some structs started getting flagged as not used
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Deserialize)]
pub enum TestStdioStream {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub enum TestEvent {
    Register(Arc<TestDescriptions>),
    Plan(TestPlan),
    Wait(usize),
    Output(Vec<u8>),
    Slow(usize, u64),
    Result(usize, TestResult, u64),
    UncaughtError(String, Box<JsError>),
    StepRegister(TestStepDescription),
    StepWait(usize),
    StepResult(usize, TestStepResult, u64),
    /// Indicates that this worker has completed running tests.
    Completed,
    /// Indicates that the user has cancelled the test run with Ctrl+C and
    /// the run should be aborted.
    Sigint,
    /// Used by the REPL to force a report to end without closing the worker
    /// or receiver.
    ForceEndReport,
}

impl TestEvent {
    // Certain messages require us to ensure that all output has been drained to ensure proper
    // interleaving of output messages.
    pub fn requires_stdio_sync(&self) -> bool {
        matches!(
            self,
            TestEvent::Plan(..)
                | TestEvent::Result(..)
                | TestEvent::StepWait(..)
                | TestEvent::StepResult(..)
                | TestEvent::UncaughtError(..)
                | TestEvent::ForceEndReport
                | TestEvent::Completed
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
    pub passed_steps: usize,
    pub failed_steps: usize,
    pub ignored_steps: usize,
    pub filtered_out: usize,
    pub measured: usize,
    pub failures: Vec<(TestFailureDescription, TestFailure)>,
    pub uncaught_errors: Vec<(String, Box<JsError>)>,
}

impl TestSummary {
    pub fn new() -> TestSummary {
        TestSummary {
            total: 0,
            passed: 0,
            failed: 0,
            ignored: 0,
            passed_steps: 0,
            failed_steps: 0,
            ignored_steps: 0,
            filtered_out: 0,
            measured: 0,
            failures: Vec::new(),
            uncaught_errors: Vec::new(),
        }
    }

    pub fn has_failed(&self) -> bool {
        self.failed > 0 || !self.failures.is_empty()
    }
}

impl Default for TestSummary {
    fn default() -> Self {
        Self::new()
    }
}

/// The test channel has been closed and cannot be used to send further messages.
#[derive(Debug, Copy, Clone, Eq, PartialEq, deno_error::JsError)]
#[class(generic)]
pub struct ChannelClosedError;

impl std::error::Error for ChannelClosedError {}

impl fmt::Display for ChannelClosedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Test channel closed")
    }
}

impl<T> From<SendError<T>> for ChannelClosedError {
    fn from(_: SendError<T>) -> Self {
        Self
    }
}

/// Sends messages from a given worker into the test stream. If multiple clones of
/// this sender are kept alive, the worker is kept alive.
///
/// Any unflushed bytes in the stdout or stderr stream associated with this sender
/// are not guaranteed to be sent on drop unless flush is explicitly called.
pub struct TestEventSender {
    pub id: usize,
    sender: UnboundedSender<(usize, TestEvent)>,
}

impl TestEventSender {
    pub fn send(&mut self, message: TestEvent) -> Result<(), ChannelClosedError> {
        Ok(self.sender.send((self.id, message))?)
    }
}

/// Polls for the next [`TestEvent`] from any worker. Events from multiple worker
/// streams may be interleaved.
pub struct TestEventReceiver {
    receiver: UnboundedReceiver<(usize, TestEvent)>,
}

impl TestEventReceiver {
    /// Receive a single test event, or `None` if no workers are alive.
    pub async fn recv(&mut self) -> Option<(usize, TestEvent)> {
        self.receiver.recv().await
    }
}

pub fn create_test_event_channel() -> (TestEventSender, TestEventReceiver) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let sender = TestEventSender { id: 0, sender: tx };
    let receiver = TestEventReceiver { receiver: rx };
    (sender, receiver)
}

extension!(
  jstz_test,
  ops = [
    op_register_test,
    op_register_test_step,
    op_register_test_hook,
    op_test_get_origin,
    op_test_event_step_wait,
    op_test_event_step_result_ok,
    op_test_event_step_result_ignored,
    op_test_event_step_result_failed,
  ],
  esm_entry_point = "ext:jstz_test/40_test.js",
  esm = [dir "src/ext/jstz_test", "40_test_common.js", "40_test.js"],
  options = {
    sender: TestEventSender,
  },
  state = |state, options| {
    state.put(options.sender);
    state.put(TestContainer::default());
  },
);

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

#[allow(clippy::too_many_arguments)]
#[op2]
fn op_register_test(
    state: &mut OpState,
    #[global] function: v8::Global<v8::Function>,
    #[string] name: String,
    ignore: bool,
    only: bool,
    sanitize_ops: bool,
    sanitize_resources: bool,
    #[string] file_name: String,
    #[smi] line_number: u32,
    #[smi] column_number: u32,
    #[buffer] ret_buf: &mut [u8],
) -> Result<(), JsErrorBox> {
    if ret_buf.len() != 4 {
        return Err(JsErrorBox::type_error(format!(
            "Invalid ret_buf length: {}",
            ret_buf.len()
        )));
    }
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let origin = state.borrow::<ModuleSpecifier>().to_string();
    let description = TestDescription {
        id,
        name,
        ignore,
        only,
        sanitize_ops,
        sanitize_resources,
        origin: origin.clone(),
        location: TestLocation {
            file_name,
            line_number,
            column_number,
        },
    };
    state
        .borrow_mut::<TestContainer>()
        .register(description, function);
    ret_buf.copy_from_slice(&(id as u32).to_le_bytes());
    Ok(())
}

#[op2]
fn op_register_test_hook(
    state: &mut OpState,
    #[string] hook_type: String,
    #[global] function: v8::Global<v8::Function>,
) -> Result<(), JsErrorBox> {
    let container = state.borrow_mut::<TestContainer>();
    container.register_hook(hook_type, function);
    Ok(())
}

#[op2]
#[string]
fn op_test_get_origin(state: &mut OpState) -> String {
    state.borrow::<ModuleSpecifier>().to_string()
}

#[op2(fast)]
#[smi]
#[allow(clippy::too_many_arguments)]
fn op_register_test_step(
    state: &mut OpState,
    #[string] name: String,
    #[string] file_name: String,
    #[smi] line_number: u32,
    #[smi] column_number: u32,
    #[smi] level: usize,
    #[smi] parent_id: usize,
    #[smi] root_id: usize,
    #[string] root_name: String,
) -> usize {
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let origin = state.borrow::<ModuleSpecifier>().to_string();
    let description = TestStepDescription {
        id,
        name,
        origin: origin.clone(),
        location: TestLocation {
            file_name,
            line_number,
            column_number,
        },
        level,
        parent_id,
        root_id,
        root_name,
    };
    let sender = state.borrow_mut::<TestEventSender>();
    sender.send(TestEvent::StepRegister(description)).ok();
    id
}

#[op2(fast)]
fn op_test_event_step_wait(state: &mut OpState, #[smi] id: usize) {
    let sender = state.borrow_mut::<TestEventSender>();
    sender.send(TestEvent::StepWait(id)).ok();
}

#[op2(fast)]
fn op_test_event_step_result_ok(
    state: &mut OpState,
    #[smi] id: usize,
    #[smi] duration: u64,
) {
    let sender = state.borrow_mut::<TestEventSender>();
    sender
        .send(TestEvent::StepResult(id, TestStepResult::Ok, duration))
        .ok();
}

#[op2(fast)]
fn op_test_event_step_result_ignored(
    state: &mut OpState,
    #[smi] id: usize,
    #[smi] duration: u64,
) {
    let sender = state.borrow_mut::<TestEventSender>();
    sender
        .send(TestEvent::StepResult(id, TestStepResult::Ignored, duration))
        .ok();
}

#[op2]
fn op_test_event_step_result_failed(
    state: &mut OpState,
    #[smi] id: usize,
    #[serde] failure: TestFailure,
    #[smi] duration: u64,
) {
    let sender = state.borrow_mut::<TestEventSender>();
    sender
        .send(TestEvent::StepResult(
            id,
            TestStepResult::Failed(failure),
            duration,
        ))
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::init_test_setup;

    async fn evaluate_test_specifiers(code: &'static str) -> TestContainer {
        // 1. Set up test runtime
        let (tx, _rx) = create_test_event_channel();

        init_test_setup! {
          runtime = runtime;
          specifier = (specifier, code);
          sink = sink;
          extensions = vec![jstz_test::init_ops_and_esm(tx)];
        }

        NEXT_ID.store(0, Ordering::SeqCst);

        // 2. Run tests specs

        // op_test_get_origin requires a ModuleSpecifier in state
        runtime.set_state(specifier.clone());

        let _ = runtime.execute_main_module(&specifier).await.unwrap();

        // 3. Collect TestContainer
        let container = std::mem::take(
            &mut *runtime
                .op_state()
                .borrow_mut()
                .borrow_mut::<TestContainer>(),
        );

        // The runtime must be explicitly dropped to avoid an assert false
        // in `InnerIsolateState::prepare_for_cleanup`.
        drop(runtime);

        container
    }

    #[tokio::test]
    async fn simple_test() {
        let container = evaluate_test_specifiers(
            r#"
                function assert(condition) {
                    if (!condition) {
                        throw new Error("Assertion failed");
                    }
                }

                Jstz.test("simple test", () => {
                    const x = 1 + 1;
                    assert(x === 2);
                });
            "#,
        )
        .await;

        assert_eq!(container.descriptions.len(), 1);
        let description = container.descriptions.tests.get(&0).unwrap();
        assert_eq!(description.name, "simple test");
    }
}
