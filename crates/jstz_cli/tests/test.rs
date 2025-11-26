#![cfg(feature = "v2_runtime")]

use std::{
    fs,
    ops::{Deref, DerefMut},
};
use tempfile::{NamedTempFile, TempDir};
use utils::jstz;

#[path = "./utils.rs"]
mod utils;

struct TestSession {
    process: utils::ProcessSession,
    _test_code: NamedTempFile,
}

impl TestSession {
    fn new(test_code: &str) -> Self {
        let temp_file = NamedTempFile::new().expect("Failed to create temp test file");
        fs::write(temp_file.path(), test_code)
            .expect("Failed to write test code to temp file");
        let process = jstz(&format!("test {}", temp_file.path().display()), None);
        Self {
            process,
            _test_code: temp_file,
        }
    }
}

impl Deref for TestSession {
    type Target = utils::ProcessSession;

    fn deref(&self) -> &Self::Target {
        &self.process
    }
}

impl DerefMut for TestSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.process
    }
}

#[test]
fn test_command_with_passing_test() {
    // Create a simple passing test file
    let mut test = TestSession::new(
        r#"
            Jstz.test("simple passing test", () => {
                const x = 1 + 1;
                if (x !== 2) {
                    throw new Error("Math is broken");
                }
            });
        "#,
    );

    let output = test.exp_eof().unwrap();

    // Verify the test ran and passed
    assert!(output.contains("simple passing test"));
    assert!(output.contains("ok") || output.contains("passed"));
}

#[test]
fn test_command_with_failing_test() {
    // Create a failing test file
    let mut test = TestSession::new(
        r#"
            Jstz.test("intentionally failing test", () => {
                throw new Error("This test is supposed to fail");
            });
        "#,
    );

    let output = test.exp_eof().unwrap();

    // Verify the test ran and failed
    assert!(output.contains("intentionally failing test"));
    assert!(output.contains("FAILED") || output.contains("failed"));
    assert!(output.contains("This test is supposed to fail"));
}

#[test]
fn test_command_with_multiple_tests() {
    // Create a test file with multiple tests
    let mut test = TestSession::new(
        r#"
            Jstz.test("test 1", () => {
                const x = 1 + 1;
                if (x !== 2) throw new Error("Test 1 failed");
            });

            Jstz.test("test 2", () => {
                const y = 2 * 2;
                if (y !== 4) throw new Error("Test 2 failed");
            });

            Jstz.test("test 3", () => {
                const z = 3 + 3;
                if (z !== 6) throw new Error("Test 3 failed");
            });
        "#,
    );

    let output = test.exp_eof().unwrap();

    // Verify all tests ran
    assert!(output.contains("test 1"));
    assert!(output.contains("test 2"));
    assert!(output.contains("test 3"));
    assert!(output.contains("3 passed") || output.contains("passed: 3"));
}

#[test]
fn test_command_with_hooks() {
    // Create a test file with hooks
    let mut test = TestSession::new(
        r#"
            let setupRan = false;
            let beforeEachCount = 0;
            let afterEachCount = 0;

            Jstz.test.beforeAll(() => {
                setupRan = true;
            });

            Jstz.test.beforeEach(() => {
                beforeEachCount++;
            });

            Jstz.test.afterEach(() => {
                afterEachCount++;
            });

            Jstz.test("test with hooks 1", () => {
                if (!setupRan) throw new Error("beforeAll did not run");
                if (beforeEachCount !== 1) throw new Error("beforeEach count is wrong");
            });

            Jstz.test("test with hooks 2", () => {
                if (!setupRan) throw new Error("beforeAll did not run");
                if (beforeEachCount !== 2) throw new Error("beforeEach count is wrong");
            });
        "#,
    );

    let output = test.exp_eof().unwrap();

    // Verify tests passed (which means hooks ran correctly)
    assert!(output.contains("test with hooks 1"));
    assert!(output.contains("test with hooks 2"));
    assert!(output.contains("2 passed") || output.contains("passed: 2"));
}

#[test]
fn test_command_with_ignored_test() {
    // Create a test file with an ignored test
    let mut test = TestSession::new(
        r#"
            Jstz.test("passing test", () => {
                const x = 1 + 1;
                if (x !== 2) throw new Error("Failed");
            });

            Jstz.test({
                name: "ignored test",
                ignore: true,
                fn: () => {
                    throw new Error("This should not run");
                }
            });
        "#,
    );

    let output = test.exp_eof().unwrap();

    // Verify one test passed and one was ignored
    assert!(output.contains("passing test"));
    assert!(output.contains("ignored test"));
    assert!(output.contains("1 passed") || output.contains("passed: 1"));
    assert!(output.contains("1 ignored") || output.contains("ignored: 1"));
}

#[test]
fn test_command_with_nonexistent_file() {
    let tmp_dir = TempDir::new().unwrap();

    // Try to run test on a file that doesn't exist
    let mut process = jstz("test /nonexistent/test/file.js", Some(tmp_dir));

    let output = process.exp_eof().unwrap();

    // Verify error message
    assert!(
        output.contains("Failed to read test file") || output.contains("No such file")
    );
}
