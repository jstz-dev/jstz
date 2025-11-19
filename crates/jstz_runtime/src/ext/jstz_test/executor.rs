// Copyright 2018-2025 the Deno authors. MIT license.

use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use deno_core::{
    error::{CoreError, JsError},
    serde_v8, v8, ModuleSpecifier, OpState, PollEventLoopOptions,
};
use tokio::time::Instant;

use crate::{
    jstz_test::{
        ChannelClosedError, TestContainer, TestDescription, TestDescriptions, TestEvent,
        TestEventSender, TestFailure, TestPlan, TestResult,
    },
    JstzRuntime,
};

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum TestExecutorError {
    #[class(inherit)]
    #[error(transparent)]
    Core(#[from] CoreError),
    #[class(inherit)]
    #[error(transparent)]
    ChannelClosed(#[from] ChannelClosedError),
    #[class(inherit)]
    #[error(transparent)]
    SerdeV8(#[from] serde_v8::Error),
}

#[derive(Clone)]
pub struct TestEventTracker {
    op_state: Rc<RefCell<OpState>>,
}

impl TestEventTracker {
    pub fn new(op_state: Rc<RefCell<OpState>>) -> Self {
        Self { op_state }
    }

    fn send_event(&self, event: TestEvent) -> Result<(), ChannelClosedError> {
        self.op_state
            .borrow_mut()
            .borrow_mut::<TestEventSender>()
            .send(event)
    }

    fn wait(&self, desc: &TestDescription) -> Result<(), ChannelClosedError> {
        self.send_event(TestEvent::Wait(desc.id))
    }

    fn ignored(&self, desc: &TestDescription) -> Result<(), ChannelClosedError> {
        self.send_event(TestEvent::Result(desc.id, TestResult::Ignored, 0))
    }

    fn cancelled(&self, desc: &TestDescription) -> Result<(), ChannelClosedError> {
        self.send_event(TestEvent::Result(desc.id, TestResult::Cancelled, 0))
    }

    fn register(
        &self,
        descriptions: Arc<TestDescriptions>,
    ) -> Result<(), ChannelClosedError> {
        self.send_event(TestEvent::Register(descriptions))
    }

    fn completed(&self) -> Result<(), ChannelClosedError> {
        self.send_event(TestEvent::Completed)
    }

    fn uncaught_error(
        &self,
        specifier: String,
        error: Box<JsError>,
    ) -> Result<(), ChannelClosedError> {
        self.send_event(TestEvent::UncaughtError(specifier, error))
    }

    fn plan(&self, plan: TestPlan) -> Result<(), ChannelClosedError> {
        self.send_event(TestEvent::Plan(plan))
    }

    fn result(
        &self,
        desc: &TestDescription,
        test_result: TestResult,
        duration: Duration,
    ) -> Result<(), ChannelClosedError> {
        self.send_event(TestEvent::Result(
            desc.id,
            test_result,
            duration.as_millis() as u64,
        ))
    }
}

async fn call_hooks<H>(
    rt: &mut JstzRuntime,
    hook_fns: impl Iterator<Item = &v8::Global<v8::Function>>,
    mut error_handler: H,
) -> Result<(), TestExecutorError>
where
    H: FnMut(CoreError) -> Result<(), TestExecutorError>,
{
    for hook_fn in hook_fns {
        let call = rt.call(hook_fn);
        let result = rt
            .with_event_loop_promise(call, PollEventLoopOptions::default())
            .await;
        if let Err(e) = result {
            error_handler(e)?;
        }
    }
    Ok(())
}
fn handle_error(
    core_error: CoreError,
    specifier: &ModuleSpecifier,
    event_tracker: &TestEventTracker,
) -> Result<(), TestExecutorError> {
    match core_error {
        CoreError::Js(js_error) => {
            event_tracker.uncaught_error(specifier.to_string(), Box::new(js_error))?;
            Ok(())
        }
        err => Err(TestExecutorError::Core(err)),
    }
}

pub async fn test_specifier(
    runtime: &mut JstzRuntime,
    specifier: ModuleSpecifier,
) -> Result<(), TestExecutorError> {
    // 1. Set up specifier for module that we're testing
    runtime.set_state(specifier.clone());

    // 2. Load and execute the module
    let _ = runtime.execute_main_module(&specifier).await.unwrap();

    let event_tracker = TestEventTracker::new(runtime.op_state());

    // 3. Extract test container (this contains the registered tests)
    let container = std::mem::take(
        &mut *runtime
            .op_state()
            .borrow_mut()
            .borrow_mut::<TestContainer>(),
    );

    println!("Descriptions: {:?}", container.descriptions);

    // 4. Run tests
    let descriptions = Arc::new(container.descriptions);
    event_tracker.register(descriptions.clone())?;

    let test_hooks = container.test_hooks;
    let test_fns = &container.test_functions;

    let mut tests_to_run = descriptions
        .tests
        .values()
        .zip(test_fns)
        .collect::<Vec<_>>();

    event_tracker.plan(TestPlan {
        origin: specifier.to_string(),
        total: tests_to_run.len(),
        filtered_out: 0,
        used_only: false,
    })?;

    // Execute beforeAll hooks first (FIFO order)
    call_hooks(runtime, test_hooks.before_all.iter(), |core_error| {
        tests_to_run = vec![];
        handle_error(core_error, &specifier, &event_tracker)
    })
    .await?;

    let mut had_uncaught_error = false;

    for (desc, test_fn) in tests_to_run {
        if desc.ignore {
            event_tracker.ignored(desc)?;
            continue;
        }

        if had_uncaught_error {
            event_tracker.cancelled(desc)?;
            continue;
        }

        event_tracker.wait(desc)?;

        let earlier = Instant::now();
        let mut before_each_hook_errored = false;

        // Execute beforeEach hooks (FIFO order)
        call_hooks(
            runtime,
            test_hooks.before_each.iter(),
            |core_error| match core_error {
                CoreError::Js(err) => {
                    before_each_hook_errored = true;
                    let test_result =
                        TestResult::Failed(TestFailure::JsError(Box::new(err)));
                    event_tracker.result(desc, test_result, earlier.elapsed())?;
                    Ok(())
                }
                err => Err(err.into()),
            },
        )
        .await?;

        // Skip test if beforeEach hook errored
        let result = if !before_each_hook_errored {
            let call = runtime.call(test_fn);
            let result = runtime
                .with_event_loop_promise(call, PollEventLoopOptions::default())
                .await;

            let result = match result {
                Ok(r) => r,
                Err(core_error) => match core_error {
                    CoreError::Js(err) => {
                        event_tracker
                            .uncaught_error(specifier.to_string(), Box::new(err))?;
                        event_tracker.cancelled(desc)?;
                        had_uncaught_error = true;
                        continue;
                    }
                    err => return Err(TestExecutorError::Core(err)),
                },
            };

            let scope = &mut runtime.handle_scope();
            let result = v8::Local::new(scope, &result);
            serde_v8::from_v8::<TestResult>(scope, result)?
        } else {
            TestResult::Ignored
        };

        if matches!(result, TestResult::Failed(_)) {
            event_tracker.result(desc, result.clone(), earlier.elapsed())?;
        }

        // Execute afterEach hooks (LIFO order)
        call_hooks(runtime, test_hooks.after_each.iter().rev(), |core_error| {
            match core_error {
                CoreError::Js(err) => {
                    let test_result =
                        TestResult::Failed(TestFailure::JsError(Box::new(err)));
                    event_tracker.result(desc, test_result, earlier.elapsed())?;
                    Ok(())
                }
                err => Err(err.into()),
            }
        })
        .await?;

        if matches!(result, TestResult::Failed(_)) {
            continue;
        }

        event_tracker.result(desc, result, earlier.elapsed())?;
    }

    event_tracker.completed()?;

    // Execute afterAll hooks last (LIFO order)
    call_hooks(runtime, test_hooks.after_all.iter().rev(), |core_error| {
        handle_error(core_error, &specifier, &event_tracker)
    })
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        init_test_setup,
        jstz_test::{create_test_event_channel, jstz_test},
    };

    use super::*;

    async fn run_test(code: &'static str) -> Vec<(usize, TestEvent)> {
        // 1. Set up test runtime
        let (sender, mut reciever) = create_test_event_channel();

        init_test_setup! {
          runtime = runtime;
          specifier = (specifier, code);
          sink = sink;
          extensions = vec![jstz_test::init_ops_and_esm(sender)];
        }

        // 2. Run tests
        test_specifier(&mut runtime, specifier)
            .await
            .expect("Test execution failed");

        drop(runtime);

        // 3. Collect events
        let mut events = vec![];
        while let Some(event) = reciever.recv().await {
            events.push(event);
        }
        events
    }

    fn assert_all_ok(events: &[(usize, TestEvent)], expected_ok_count: usize) {
        let mut ok_count = 0;
        for (_, event) in events {
            if let TestEvent::Result(_, result, _) = event {
                assert_eq!(result, &TestResult::Ok);
                ok_count += 1;
            }
        }
        assert_eq!(ok_count, expected_ok_count);
    }

    fn assert_all_failed(events: &[(usize, TestEvent)], expected_fail_count: usize) {
        let mut fail_count = 0;
        for (_, event) in events {
            if let TestEvent::Result(_, result, _) = event {
                assert!(matches!(result, TestResult::Failed(_)));
                fail_count += 1;
            }
        }
        assert_eq!(fail_count, expected_fail_count);
    }

    #[tokio::test]
    async fn simple_test() {
        let events = run_test(
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

        assert_eq!(events.len(), 5); // Register, Plan, Wait, Result, Completed

        match &events[3].1 {
            TestEvent::Result(_, result, _) => {
                assert_eq!(result, &TestResult::Ok);
            }
            _ => panic!("Expected TestEvent::Result"),
        }
    }

    #[tokio::test]
    async fn test_failure() {
        let events = run_test(
            r#"
                Jstz.test("failing test", () => {
                    throw new Error("Test intentionally failed");
                });
            "#,
        )
        .await;

        // Verify test failed
        assert_all_failed(&events, 1);
    }

    #[tokio::test]
    async fn test_ignored() {
        let events = run_test(
            r#"
                Jstz.test({
                    name: "ignored test",
                    ignore: true,
                    fn: () => {
                        throw new Error("This should not run");
                    }
                });
            "#,
        )
        .await;

        // Verify test was ignored
        let result_event = events
            .iter()
            .find(|(_, e)| matches!(e, TestEvent::Result(..)));
        assert!(result_event.is_some());
        match &result_event.unwrap().1 {
            TestEvent::Result(_, result, _) => {
                assert_eq!(result, &TestResult::Ignored);
            }
            _ => panic!("Expected TestEvent::Result"),
        }
    }

    #[tokio::test]
    async fn multiple_tests() {
        let events = run_test(
            r#"
                Jstz.test("test 1", () => {
                    const x = 1 + 1;
                    if (x !== 2) throw new Error("Math is broken");
                });

                Jstz.test("test 2", () => {
                    const y = 2 * 2;
                    if (y !== 4) throw new Error("Math is broken");
                });

                Jstz.test("test 3", () => {
                    const z = 3 + 3;
                    if (z !== 6) throw new Error("Math is broken");
                });
            "#,
        )
        .await;

        // Count test results
        assert_all_ok(&events, 3);
    }

    #[tokio::test]
    async fn test_before_all_hook() {
        let events = run_test(
            r#"
                let setupComplete = false;

                Jstz.test.beforeAll(() => {
                    setupComplete = true;
                });

                Jstz.test("test should see setup", () => {
                    if (!setupComplete) throw new Error("beforeAll did not run");
                });
            "#,
        )
        .await;

        // Verify test passed (which means beforeAll ran successfully)
        assert_all_ok(&events, 1);
    }

    #[tokio::test]
    async fn test_after_all_hook() {
        let events = run_test(
            r#"
                let testRan = false;

                Jstz.test.afterAll(() => {
                    if (!testRan) throw new Error("Test did not run before afterAll");
                });

                Jstz.test("simple test", () => {
                    testRan = true;
                });
            "#,
        )
        .await;

        // Verify test passed and completed event was sent (afterAll didn't error)
        assert_all_ok(&events, 1);
    }

    #[tokio::test]
    async fn test_before_each_hook() {
        let events = run_test(
            r#"
                let counter = 0;

                Jstz.test.beforeEach(() => {
                    counter++;
                });

                Jstz.test("test 1", () => {
                    if (counter !== 1) throw new Error("beforeEach should have run once");
                });

                Jstz.test("test 2", () => {
                    if (counter !== 2) throw new Error("beforeEach should have run twice");
                });
            "#,
        )
        .await;

        // Verify both tests passed
        assert_all_ok(&events, 2);
    }

    #[tokio::test]
    async fn test_after_each_hook() {
        let events = run_test(
            r#"
                let cleanupCount = 0;

                Jstz.test.afterAll(() => {
                    if (cleanupCount !== 2) throw new Error("afterAll should run after all tests");
                });

                Jstz.test.afterEach(() => {
                    cleanupCount++;
                });

                Jstz.test("test 1", () => {
                    // cleanupCount should be 0 before first test
                    if (cleanupCount !== 0) throw new Error("afterEach ran too early");
                });

                Jstz.test("test 2", () => {
                    // cleanupCount should be 1 after first test
                    if (cleanupCount !== 1) throw new Error("afterEach should have run once");
                });
            "#,
        )
        .await;

        // Verify both tests passed
        assert_all_ok(&events, 2);
    }

    #[tokio::test]
    async fn test_before_all_failure_cancels_tests() {
        let events = run_test(
            r#"
                Jstz.test.beforeAll(() => {
                    throw new Error("Setup failed!");
                });

                Jstz.test("test 1", () => {
                    // This should never run
                });

                Jstz.test("test 2", () => {
                    // This should never run either
                });
            "#,
        )
        .await;

        // Should have an uncaught error from beforeAll
        let uncaught = events
            .iter()
            .find(|(_, e)| matches!(e, TestEvent::UncaughtError(..)));
        assert!(uncaught.is_some());

        // No test results should be recorded since beforeAll failed
        let result_count = events
            .iter()
            .filter(|(_, e)| matches!(e, TestEvent::Result(..)))
            .count();
        assert_eq!(result_count, 0);
    }

    #[tokio::test]
    async fn test_before_each_failure() {
        let events = run_test(
            r#"
                let shouldFail = true;

                Jstz.test.beforeEach(() => {
                    if (shouldFail) throw new Error("beforeEach failed!");
                });

                Jstz.test("test 1", () => {
                    // This should not run because beforeEach fails
                });

                Jstz.test("test 2", () => {
                    shouldFail = false;
                    // This should also not run
                });
            "#,
        )
        .await;

        // Both tests are ignored.
        let ignored_count = events
            .iter()
            .filter(|(_, e)| matches!(e, TestEvent::Result(_, TestResult::Ignored, _)))
            .count();
        assert_eq!(ignored_count, 2);

        // 2 failures from beforeEach
        let fail_count = events
            .iter()
            .filter(|(_, e)| matches!(e, TestEvent::Result(_, TestResult::Failed(_), _)))
            .count();
        assert_eq!(fail_count, 2);
    }

    #[tokio::test]
    async fn test_after_each_failure() {
        let events = run_test(
            r#"
                Jstz.test.afterEach(() => {
                    throw new Error("afterEach cleanup failed!");
                });

                Jstz.test("test 1", () => {
                    // Test itself passes
                });
            "#,
        )
        .await;

        // Test should pass
        let ok_count = events
            .iter()
            .filter(|(_, e)| matches!(e, TestEvent::Result(_, TestResult::Ok, _)))
            .count();
        assert_eq!(ok_count, 1);

        // But there should be an uncaught error from afterEach (reported as a failed test)
        let failed_count = events
            .iter()
            .filter(|(_, e)| matches!(e, TestEvent::Result(_, TestResult::Failed(_), _)))
            .count();
        assert_eq!(failed_count, 1);
    }

    #[tokio::test]
    async fn test_after_all_failure() {
        let events = run_test(
            r#"
                Jstz.test.afterAll(() => {
                    throw new Error("afterAll cleanup failed!");
                });

                Jstz.test("test 1", () => {
                    // Test passes
                });
            "#,
        )
        .await;

        // Test itself should pass
        assert_all_ok(&events, 1);

        // But there should be an uncaught error from afterAll
        let uncaught = events
            .iter()
            .find(|(_, e)| matches!(e, TestEvent::UncaughtError(..)));
        assert!(uncaught.is_some());
    }
}
