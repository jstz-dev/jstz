CLI_KERNEL_PATH := crates/jstz_cli/jstz_kernel.wasm

.PHONY: all
all: test build check

.PHONY: build
build: build-cli-kernel
	@cargo build --release

.PHONY: build-bridge
build-bridge:
	@ligo compile contract --no-warn contracts/jstz_bridge.mligo \
		--module "Jstz_bridge" > contracts/jstz_bridge.tz 
	@ligo compile contract contracts/jstz_native_bridge.mligo > contracts/jstz_native_bridge.tz
	@ligo compile contract --no-warn contracts/exchanger.mligo > contracts/exchanger.tz

.PHONY: build-kernel
build-kernel:
	@cargo build --package jstz_kernel --target wasm32-unknown-unknown --release

.PHONY: build-cli-kernel
build-cli-kernel: build-kernel
	@cp target/wasm32-unknown-unknown/release/jstz_kernel.wasm crates/jstz_cli/jstz_kernel.wasm

.PHONY: build-cli
build-cli: build-cli-kernel
	@cargo build --package jstz_cli --release

.PHONY: build-deps
build-deps:
	@rustup target add wasm32-unknown-unknown

.PHONY: build-dev-deps
build-dev-deps: build-deps
	@rustup component add rustfmt clippy

.PHONY: build-sdk-wasm-pkg
build-sdk-wasm-pkg:
	@cd crates/jstz_sdk && wasm-pack build --target bundler --release

.PHONY: test
test: test-unit test-int

.PHONY: test-unit
test-unit:
# --lib only runs unit tests in library crates
# --bins only runs unit tests in binary crates
	@cargo test --lib --bins 

.PHONY: test-int
test-int:
# --test only runs a specified integration test (a test in /tests).
#        the glob pattern is used to match all integration tests
# 
# FIXME(https://linear.app/tezos/issue/JSTZ-46): 
# Currently this runs the test for `test_nested_transactions`. This test should 
# be moved to an inline-test in the `jstz_core` crate to avoid this.  
	@cargo test --test "*"

.PHONY: cov 
cov:
# TODO(https://linear.app/tezos/issue/JSTZ-47): 
# This will only generate a coverage report for unit tests. We should add coverage 
# for integration tests as well.
	@cargo llvm-cov --lib --bins --html --open 

.PHONY: check
check: lint fmt

.PHONY: clean
clean:
	@cargo clean
	@rm -f result
	@rm -rf logs

.PHONY: fmt-nix-check
fmt-nix-check:
	@alejandra check ./

.PHONY: fmt-nix
fmt-nix:
	@alejandra ./

.PHONY: fmt-rust-check
fmt-rust-check:
	@cargo fmt --check

.PHONY: fmt-rust
fmt-rust:
	@cargo fmt

.PHONY: fmt-js-check
fmt-js-check:
	npm run check:format

.PHONY: fmt-js
fmt-js:
	npm run format

.PHONY: fmt
fmt: fmt-nix fmt-rust fmt-js

.PHONY: fmt-check
fmt-check: fmt-nix-check fmt-rust-check fmt-js-check

.PHONY: lint
lint:
	@touch $(CLI_KERNEL_PATH)
	@cargo clippy -- --no-deps -D warnings -A clippy::let_underscore_future -A clippy::module_inception -A clippy::op_ref -A clippy::manual_strip -A clippy::missing_safety_doc -A clippy::slow_vector_initialization -A clippy::empty_loop -A clippy::collapsible-match
	@rm -f $(CLI_KERNEL_PATH)