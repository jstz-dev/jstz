// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::Write;
use std::{collections::HashSet, time::Duration};

use deno_core::error::JsError;
use deno_terminal::colors;
use indexmap::IndexMap;
use jstz_runtime::ext::jstz_test::{
    TestDescription, TestPlan, TestResult, TestStepDescription, TestStepResult,
};
use jstz_runtime::jstz_test::{
    TestFailure, TestFailureDescription, TestFailureFormatOptions, TestSummary,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use url::Url;

pub trait TestReporter {
    fn report_register(&mut self, description: &TestDescription);
    fn report_plan(&mut self, plan: &TestPlan);
    fn report_wait(&mut self, description: &TestDescription);
    fn report_slow(&mut self, description: &TestDescription, elapsed: u64);
    fn report_output(&mut self, output: &[u8]);
    fn report_result(
        &mut self,
        description: &TestDescription,
        result: &TestResult,
        elapsed: u64,
    );
    fn report_uncaught_error(&mut self, origin: &str, error: Box<JsError>);
    fn report_step_register(&mut self, description: &TestStepDescription);
    fn report_step_wait(&mut self, description: &TestStepDescription);
    fn report_step_result(
        &mut self,
        desc: &TestStepDescription,
        result: &TestStepResult,
        elapsed: u64,
        tests: &IndexMap<usize, TestDescription>,
        test_steps: &IndexMap<usize, TestStepDescription>,
    );
    fn report_summary(
        &mut self,
        elapsed: &Duration,
        tests: &IndexMap<usize, TestDescription>,
        test_steps: &IndexMap<usize, TestStepDescription>,
    );
    fn report_sigint(
        &mut self,
        tests_pending: &HashSet<usize>,
        tests: &IndexMap<usize, TestDescription>,
        test_steps: &IndexMap<usize, TestStepDescription>,
    );
    fn report_completed(&mut self);
    fn flush_report(
        &mut self,
        elapsed: &Duration,
        tests: &IndexMap<usize, TestDescription>,
        test_steps: &IndexMap<usize, TestStepDescription>,
    ) -> anyhow::Result<()>;
}

mod common {
    use std::borrow::Cow;

    use super::*;

    pub(super) fn format_test_step_ancestry(
        desc: &TestStepDescription,
        tests: &IndexMap<usize, TestDescription>,
        test_steps: &IndexMap<usize, TestStepDescription>,
    ) -> String {
        let root;
        let mut ancestor_names = vec![];
        let mut current_desc = desc;
        loop {
            if let Some(step_desc) = test_steps.get(&current_desc.parent_id) {
                ancestor_names.push(&step_desc.name);
                current_desc = step_desc;
            } else {
                root = tests.get(&current_desc.parent_id).unwrap();
                break;
            }
        }
        ancestor_names.reverse();
        let mut result = String::new();
        result.push_str(&root.name);
        result.push_str(" ... ");
        for name in ancestor_names {
            result.push_str(name);
            result.push_str(" ... ");
        }
        result.push_str(&desc.name);
        result
    }

    pub fn format_test_for_summary(cwd: &Url, desc: &TestFailureDescription) -> String {
        format!(
            "{} {}",
            &desc.name,
            colors::gray(format!(
                "=> {}:{}:{}",
                to_relative_path_or_remote_url(cwd, &desc.location.file_name),
                desc.location.line_number,
                desc.location.column_number
            ))
        )
    }

    pub fn to_relative_path_or_remote_url(_cwd: &Url, path_or_url: &str) -> String {
        if !Url::parse(path_or_url).is_ok() {
            return "<anonymous>".to_string();
        };

        path_or_url.to_string()
    }

    pub fn format_test_step_for_summary(
        cwd: &Url,
        desc: &TestStepDescription,
        tests: &IndexMap<usize, TestDescription>,
        test_steps: &IndexMap<usize, TestStepDescription>,
    ) -> String {
        let long_name = format_test_step_ancestry(desc, tests, test_steps);
        format!(
            "{} {}",
            long_name,
            colors::gray(format!(
                "=> {}:{}:{}",
                to_relative_path_or_remote_url(cwd, &desc.location.file_name),
                desc.location.line_number,
                desc.location.column_number
            ))
        )
    }

    pub(super) fn report_sigint(
        writer: &mut dyn std::io::Write,
        cwd: &Url,
        tests_pending: &HashSet<usize>,
        tests: &IndexMap<usize, TestDescription>,
        test_steps: &IndexMap<usize, TestStepDescription>,
    ) {
        if tests_pending.is_empty() {
            return;
        }
        let mut formatted_pending = BTreeSet::new();
        for id in tests_pending {
            if let Some(desc) = tests.get(id) {
                formatted_pending.insert(format_test_for_summary(cwd, &desc.into()));
            }
            if let Some(desc) = test_steps.get(id) {
                formatted_pending
                    .insert(format_test_step_for_summary(cwd, desc, tests, test_steps));
            }
        }
        writeln!(
            writer,
            "\n{} The following tests were pending:\n",
            colors::intense_blue("SIGINT")
        )
        .ok();
        for entry in formatted_pending {
            writeln!(writer, "{}", entry).ok();
        }
        writeln!(writer).ok();
    }

    pub(super) fn report_summary(
        writer: &mut dyn std::io::Write,
        cwd: &Url,
        summary: &TestSummary,
        elapsed: &Duration,
        options: &TestFailureFormatOptions,
    ) {
        if !summary.failures.is_empty() || !summary.uncaught_errors.is_empty() {
            #[allow(clippy::type_complexity)] // Type alias doesn't look better here
            let mut failures_by_origin: BTreeMap<
                String,
                (
                    Vec<(&TestFailureDescription, &TestFailure)>,
                    Option<&JsError>,
                ),
            > = BTreeMap::default();
            let mut failure_titles = vec![];
            for (description, failure) in &summary.failures {
                let (failures, _) = failures_by_origin
                    .entry(description.origin.clone())
                    .or_default();
                failures.push((description, failure));
            }

            for (origin, js_error) in &summary.uncaught_errors {
                let (_, uncaught_error) =
                    failures_by_origin.entry(origin.clone()).or_default();
                let _ = uncaught_error.insert(js_error.as_ref());
            }

            // note: the trailing whitespace is intentional to get a red background
            writeln!(writer, "\n{}\n", colors::white_bold_on_red(" ERRORS ")).ok();
            for (origin, (failures, uncaught_error)) in failures_by_origin {
                for (description, failure) in failures {
                    if !failure.hide_in_summary() {
                        let failure_title = format_test_for_summary(cwd, description);
                        writeln!(writer, "{}", &failure_title).ok();
                        writeln!(
                            writer,
                            "{}: {}",
                            colors::red_bold("error"),
                            format_failure(failure, options)
                        )
                        .ok();
                        writeln!(writer).ok();
                        failure_titles.push(failure_title);
                    }
                }
                if let Some(js_error) = uncaught_error {
                    let failure_title = format!(
                        "{} (uncaught error)",
                        to_relative_path_or_remote_url(cwd, &origin)
                    );
                    writeln!(writer, "{}", &failure_title).ok();
                    writeln!(
                        writer,
                        "{}: {}",
                        colors::red_bold("error"),
                        format_test_error(js_error, options)
                    )
                    .ok();
                    writeln!(writer, "This error was not caught from a test and caused the test runner to fail on the referenced module.").ok();
                    writeln!(writer, "It most likely originated from a dangling promise, event/timeout handler or top-level code.").ok();
                    writeln!(writer).ok();
                    failure_titles.push(failure_title);
                }
            }
            // note: the trailing whitespace is intentional to get a red background
            writeln!(writer, "{}\n", colors::white_bold_on_red(" FAILURES ")).ok();
            for failure_title in failure_titles {
                writeln!(writer, "{failure_title}").ok();
            }
        }

        let status = if summary.has_failed() {
            colors::red("FAILED").to_string()
        } else {
            colors::green("ok").to_string()
        };

        let get_steps_text = |count: usize| -> String {
            if count == 0 {
                String::new()
            } else if count == 1 {
                " (1 step)".to_string()
            } else {
                format!(" ({count} steps)")
            }
        };

        let mut summary_result = String::new();

        summary_result.push_str(&format!(
            "{} passed{} | {} failed{}",
            summary.passed,
            get_steps_text(summary.passed_steps),
            summary.failed,
            get_steps_text(summary.failed_steps),
        ));

        let ignored_steps = get_steps_text(summary.ignored_steps);
        if summary.ignored > 0 || !ignored_steps.is_empty() {
            summary_result
                .push_str(&format!(" | {} ignored{}", summary.ignored, ignored_steps));
        }

        if summary.measured > 0 {
            summary_result.push_str(&format!(" | {} measured", summary.measured,));
        }

        if summary.filtered_out > 0 {
            summary_result
                .push_str(&format!(" | {} filtered out", summary.filtered_out,));
        };

        writeln!(
            writer,
            "\n{} | {} {}",
            status,
            summary_result,
            colors::gray(format!("({})", elapsed.as_millis())),
        )
        .ok();
    }

    // TODO(alistair): Improve the formatting of JS errors
    pub fn format_test_error(
        js_error: &JsError,
        _options: &TestFailureFormatOptions,
    ) -> String {
        let mut js_error = js_error.clone();
        js_error.exception_message = js_error
            .exception_message
            .trim_start_matches("Uncaught ")
            .to_string();

        return js_error.exception_message;
    }

    pub fn format_failure(
        failure: &TestFailure,
        options: &TestFailureFormatOptions,
    ) -> Cow<'static, str> {
        match failure {
          TestFailure::JsError(js_error) => {
            Cow::Owned(format_test_error(js_error, options))
          }
          TestFailure::FailedSteps(1) => Cow::Borrowed("1 test step failed."),
          TestFailure::FailedSteps(n) => {
            Cow::Owned(format!("{} test steps failed.", n))
          }
          TestFailure::IncompleteSteps => Cow::Borrowed(
            "Completed while steps were still running. Ensure all steps are awaited with `await t.step(...)`.",
          ),
          TestFailure::Incomplete => Cow::Borrowed(
            "Didn't complete before parent. Await step with `await t.step(...)`.",
          ),
          TestFailure::Leaked(details, trailer_notes) => {
            let mut f = String::new();
            f.push_str("Leaks detected:");
            for detail in details {
              f.push_str(&format!("\n  - {}", detail));
            }
            for trailer in trailer_notes {
              f.push_str(&format!("\n{}", trailer));
            }
            Cow::Owned(f)
          }
          TestFailure::OverlapsWithSanitizers(long_names) => {
            let mut f = String::new();
            f.push_str("Started test step while another test step with sanitizers was running:");
            for long_name in long_names {
              f.push_str(&format!("\n  * {}", long_name));
            }
            Cow::Owned(f)
          }
          TestFailure::HasSanitizersAndOverlaps(long_names) => {
            let mut f = String::new();
            f.push_str("Started test step with sanitizers while another test step was running:");
            for long_name in long_names {
              f.push_str(&format!("\n  * {}", long_name));
            }
            Cow::Owned(f)
          }
        }
    }

    pub fn format_failure_label(failure: &TestFailure) -> String {
        match failure {
            TestFailure::Incomplete => colors::gray("INCOMPLETE").to_string(),
            _ => colors::red("FAILED").to_string(),
        }
    }

    pub fn format_inline_summary(failure: &TestFailure) -> Option<String> {
        match failure {
            TestFailure::FailedSteps(1) => Some("due to 1 failed step".to_string()),
            TestFailure::FailedSteps(n) => Some(format!("due to {} failed steps", n)),
            TestFailure::IncompleteSteps => Some("due to incomplete steps".to_string()),
            _ => None,
        }
    }
}

pub struct PrettyTestReporter {
    parallel: bool,
    echo_output: bool,
    in_new_line: bool,
    phase: &'static str,
    filter: bool,
    repl: bool,
    scope_test_id: Option<usize>,
    cwd: Url,
    did_have_user_output: bool,
    started_tests: bool,
    ended_tests: bool,
    child_results_buffer:
        HashMap<usize, IndexMap<usize, (TestStepDescription, TestStepResult, u64)>>,
    summary: TestSummary,
    writer: Box<dyn std::io::Write>,
    failure_format_options: TestFailureFormatOptions,
}

impl PrettyTestReporter {
    pub fn new(
        parallel: bool,
        echo_output: bool,
        filter: bool,
        repl: bool,
        cwd: Url,
        failure_format_options: TestFailureFormatOptions,
    ) -> PrettyTestReporter {
        PrettyTestReporter {
            parallel,
            echo_output,
            in_new_line: true,
            phase: "",
            filter,
            repl,
            scope_test_id: None,
            cwd,
            did_have_user_output: false,
            started_tests: false,
            ended_tests: false,
            child_results_buffer: Default::default(),
            summary: TestSummary::new(),
            writer: Box::new(std::io::stdout()),
            failure_format_options,
        }
    }

    fn force_report_wait(&mut self, description: &TestDescription) {
        if !self.in_new_line {
            writeln!(&mut self.writer).ok();
        }
        if self.parallel {
            write!(
                &mut self.writer,
                "{}",
                colors::gray(format!(
                    "{} => ",
                    common::to_relative_path_or_remote_url(
                        &self.cwd,
                        &description.origin
                    )
                ))
            )
            .ok();
        }
        write!(&mut self.writer, "{} ...", description.name).ok();
        self.in_new_line = false;
        // flush for faster feedback when line buffered
        std::io::stdout().flush().ok();
        self.scope_test_id = Some(description.id);
    }

    fn force_report_step_wait(&mut self, description: &TestStepDescription) {
        self.write_output_end();
        if !self.in_new_line {
            writeln!(&mut self.writer).ok();
        }
        write!(
            &mut self.writer,
            "{}{} ...",
            "  ".repeat(description.level),
            description.name
        )
        .ok();
        self.in_new_line = false;
        // flush for faster feedback when line buffered
        std::io::stdout().flush().ok();
        self.scope_test_id = Some(description.id);
    }

    fn force_report_step_result(
        &mut self,
        description: &TestStepDescription,
        result: &TestStepResult,
        elapsed: u64,
    ) {
        self.write_output_end();
        if self.in_new_line || self.scope_test_id != Some(description.id) {
            self.force_report_step_wait(description);
        }

        if !self.parallel {
            let child_results = self
                .child_results_buffer
                .remove(&description.id)
                .unwrap_or_default();
            for (desc, result, elapsed) in child_results.values() {
                self.force_report_step_result(desc, result, *elapsed);
            }
            if !child_results.is_empty() {
                self.force_report_step_wait(description);
            }
        }

        let status = match &result {
            TestStepResult::Ok => colors::green("ok").to_string(),
            TestStepResult::Ignored => colors::yellow("ignored").to_string(),
            TestStepResult::Failed(failure) => common::format_failure_label(failure),
        };
        write!(&mut self.writer, " {status}").ok();
        if let TestStepResult::Failed(failure) = result {
            if let Some(inline_summary) = common::format_inline_summary(failure) {
                write!(&mut self.writer, " ({})", inline_summary).ok();
            }
        }
        if !matches!(result, TestStepResult::Failed(TestFailure::Incomplete)) {
            write!(
                &mut self.writer,
                " {}",
                colors::gray(format!("({})", elapsed))
            )
            .ok();
        }
        writeln!(&mut self.writer).ok();
        self.in_new_line = true;
        if self.parallel {
            self.scope_test_id = None;
        } else {
            self.scope_test_id = Some(description.parent_id);
        }
        self.child_results_buffer
            .entry(description.parent_id)
            .or_default()
            .shift_remove(&description.id);
    }

    fn write_output_end(&mut self) {
        if self.did_have_user_output {
            writeln!(
                &mut self.writer,
                "{}",
                colors::gray(format!("----- {}output end -----", self.phase))
            )
            .ok();
            self.in_new_line = true;
            self.did_have_user_output = false;
        }
    }
}

impl TestReporter for PrettyTestReporter {
    fn report_register(&mut self, _description: &TestDescription) {}
    fn report_plan(&mut self, plan: &TestPlan) {
        self.write_output_end();
        self.summary.total += plan.total;
        self.summary.filtered_out += plan.filtered_out;
        if self.repl {
            return;
        }
        if self.parallel || (self.filter && plan.total == 0) {
            return;
        }
        let inflection = if plan.total == 1 { "test" } else { "tests" };
        writeln!(
            &mut self.writer,
            "{}",
            colors::gray(format!(
                "running {} {} from {}",
                plan.total,
                inflection,
                common::to_relative_path_or_remote_url(&self.cwd, &plan.origin)
            ))
        )
        .ok();
        self.in_new_line = true;
    }

    fn report_wait(&mut self, description: &TestDescription) {
        self.write_output_end();
        if !self.parallel {
            self.force_report_wait(description);
        }
        self.started_tests = true;
    }

    fn report_slow(&mut self, description: &TestDescription, elapsed: u64) {
        writeln!(
            &mut self.writer,
            "{}",
            colors::yellow_bold(format!(
                "'{}' has been running for over {}",
                description.name,
                colors::gray(format!("({})", elapsed)),
            ))
        )
        .ok();
    }
    fn report_output(&mut self, output: &[u8]) {
        if !self.echo_output {
            return;
        }

        if !self.did_have_user_output {
            self.did_have_user_output = true;
            if !self.in_new_line {
                writeln!(&mut self.writer).ok();
            }
            self.phase = if !self.started_tests {
                "pre-test "
            } else if self.ended_tests {
                "post-test "
            } else {
                ""
            };
            writeln!(
                &mut self.writer,
                "{}",
                colors::gray(format!("------- {}output -------", self.phase))
            )
            .ok();
            self.in_new_line = true;
        }

        // output everything to stdout in order to prevent
        // stdout and stderr racing
        std::io::stdout().write_all(output).ok();
    }

    fn report_result(
        &mut self,
        description: &TestDescription,
        result: &TestResult,
        elapsed: u64,
    ) {
        match &result {
            TestResult::Ok => {
                self.summary.passed += 1;
            }
            TestResult::Ignored => {
                self.summary.ignored += 1;
            }
            TestResult::Failed(failure) => {
                self.summary.failed += 1;
                self.summary
                    .failures
                    .push((description.into(), failure.clone()));
            }
            TestResult::Cancelled => {
                self.summary.failed += 1;
            }
        }

        if self.parallel {
            self.force_report_wait(description);
        }

        self.write_output_end();
        if self.in_new_line || self.scope_test_id != Some(description.id) {
            self.force_report_wait(description);
        }

        let status = match result {
            TestResult::Ok => colors::green("ok").to_string(),
            TestResult::Ignored => colors::yellow("ignored").to_string(),
            TestResult::Failed(failure) => common::format_failure_label(failure),
            TestResult::Cancelled => colors::gray("cancelled").to_string(),
        };
        write!(&mut self.writer, " {status}").ok();
        if let TestResult::Failed(failure) = result {
            if let Some(inline_summary) = common::format_inline_summary(failure) {
                write!(&mut self.writer, " ({})", inline_summary).ok();
            }
        }
        writeln!(
            &mut self.writer,
            " {}",
            colors::gray(format!("({})", elapsed))
        )
        .ok();
        self.in_new_line = true;
        self.scope_test_id = None;
    }

    fn report_uncaught_error(&mut self, origin: &str, error: Box<JsError>) {
        self.summary.failed += 1;
        self.summary
            .uncaught_errors
            .push((origin.to_string(), error));

        if !self.in_new_line {
            writeln!(&mut self.writer).ok();
        }
        writeln!(
            &mut self.writer,
            "Uncaught error from {} {}",
            common::to_relative_path_or_remote_url(&self.cwd, origin),
            colors::red("FAILED")
        )
        .ok();
        self.in_new_line = true;
        self.did_have_user_output = false;
    }

    fn report_step_register(&mut self, _description: &TestStepDescription) {}

    fn report_step_wait(&mut self, description: &TestStepDescription) {
        if !self.parallel && self.scope_test_id == Some(description.parent_id) {
            self.force_report_step_wait(description);
        }
    }

    fn report_step_result(
        &mut self,
        desc: &TestStepDescription,
        result: &TestStepResult,
        elapsed: u64,
        tests: &IndexMap<usize, TestDescription>,
        test_steps: &IndexMap<usize, TestStepDescription>,
    ) {
        match &result {
            TestStepResult::Ok => {
                self.summary.passed_steps += 1;
            }
            TestStepResult::Ignored => {
                self.summary.ignored_steps += 1;
            }
            TestStepResult::Failed(failure) => {
                self.summary.failed_steps += 1;
                self.summary.failures.push((
                    TestFailureDescription {
                        id: desc.id,
                        name: common::format_test_step_ancestry(desc, tests, test_steps),
                        origin: desc.origin.clone(),
                        location: desc.location.clone(),
                    },
                    failure.clone(),
                ))
            }
        }

        if self.parallel {
            self.write_output_end();
            write!(
                &mut self.writer,
                "{} {} ...",
                colors::gray(format!(
                    "{} =>",
                    common::to_relative_path_or_remote_url(&self.cwd, &desc.origin)
                )),
                common::format_test_step_ancestry(desc, tests, test_steps)
            )
            .ok();
            self.in_new_line = false;
            self.scope_test_id = Some(desc.id);
            self.force_report_step_result(desc, result, elapsed);
        } else {
            let sibling_results =
                self.child_results_buffer.entry(desc.parent_id).or_default();
            if self.scope_test_id == Some(desc.id)
                || self.scope_test_id == Some(desc.parent_id)
            {
                let sibling_results = std::mem::take(sibling_results);
                self.force_report_step_result(desc, result, elapsed);
                // Flush buffered sibling results.
                for (desc, result, elapsed) in sibling_results.values() {
                    self.force_report_step_result(desc, result, *elapsed);
                }
            } else {
                sibling_results.insert(desc.id, (desc.clone(), result.clone(), elapsed));
            }
        }
    }

    fn report_summary(
        &mut self,
        elapsed: &Duration,
        _tests: &IndexMap<usize, TestDescription>,
        _test_steps: &IndexMap<usize, TestStepDescription>,
    ) {
        self.write_output_end();
        common::report_summary(
            &mut self.writer,
            &self.cwd,
            &self.summary,
            elapsed,
            &self.failure_format_options,
        );
        if !self.repl {
            writeln!(&mut self.writer).ok();
        }
        self.in_new_line = true;
    }

    fn report_sigint(
        &mut self,
        tests_pending: &HashSet<usize>,
        tests: &IndexMap<usize, TestDescription>,
        test_steps: &IndexMap<usize, TestStepDescription>,
    ) {
        common::report_sigint(
            &mut self.writer,
            &self.cwd,
            tests_pending,
            tests,
            test_steps,
        );
        self.in_new_line = true;
    }

    fn report_completed(&mut self) {
        self.write_output_end();
        self.ended_tests = true;
    }

    fn flush_report(
        &mut self,
        _elapsed: &Duration,
        _tests: &IndexMap<usize, TestDescription>,
        _test_steps: &IndexMap<usize, TestStepDescription>,
    ) -> anyhow::Result<()> {
        self.writer.flush().ok();
        Ok(())
    }
}
