# Description

<!-- Please be sure to link the associated Runtime API task here. -->

**Related issue**: [Name](link)

<!-- If this PR has dependencies, please link them here. -->
<!-- **Dependencies**: -->

<!-- Describe your changes in detail. -->

# Manual testing

<!-- Describe how reviewers and approvers can manually test this PR. -->

```sh
nix develop
cargo run --bin jstz -- repl
>> <!-- Provide reviewers with example usages of the Runtime API -->
```

# Testing

<!-- Describe how reviewers and approves can manually run the unit tests for this PR. -->

<!-- Additionally, describe the employed testing strategy -->

<!--

A possible testing strategy could involve:
- Copy test from other runtime (e.g. for TextEncoder from [bun](https://github.com/oven-sh/bun) can be found [here](https://github.com/oven-sh/bun/blob/main/test/js/web/encoding/text-encoder.test.js))
- Remove test harness imports
- Replace unit tests (described by it in bun) with a function (of type () => void)
- Replace expects with console.asserts.
- Define a main function for the tests which can be loaded into the REPL (currently by copying and pasting the file) and executed.

-->

# Checklist

- [ ] Changes follow the existing code style (use `make fmt-check` to check)
- [ ] Tests for changes have been added
- [ ] Internal documentation has been added (if appropriate)
- [ ] Testing instructions have been added to PR
