# In some situations we might want to override the default profile (release) (e.g. in CI)
PROFILE ?= release
PROFILE_OPT := --profile $(PROFILE)

# Frustratingly, for the dev profile, /target/debug is used. For all other profiles,
# /target/$(PROFILE) is used. This is a workaround to ensure that the correct target
# directory is used for the dev profile.
ifeq ($(PROFILE), dev)
	PROFILE_TARGET_DIR := debug
else
	PROFILE_TARGET_DIR := $(PROFILE)
endif

JSTZD_KERNEL_PATH := crates/jstzd/resources/jstz_rollup/jstz_kernel.wasm

.PHONY: all
all: build test build-v2 test-v2 check

.PHONY: build
build: build-jstzd-kernel
	@cargo build $(PROFILE_OPT)

build-v2: build-jstzd-kernel
	@cargo build $(PROFILE_OPT) --features v2_runtime

.PHONY: build-bridge
build-bridge:
	@ligo compile contract --no-warn contracts/jstz_bridge.mligo \
		--module "Jstz_bridge" > contracts/jstz_bridge.tz
	@ligo compile contract contracts/jstz_native_bridge.mligo > contracts/jstz_native_bridge.tz
	@ligo compile contract --no-warn contracts/exchanger.mligo > contracts/exchanger.tz
	@ligo compile contract --no-warn contracts/jstz_fa_bridge.mligo > contracts/jstz_fa_bridge.tz
	@ligo compile contract --no-warn contracts/examples/fa_ticketer/fa_ticketer.mligo > contracts/examples/fa_ticketer/fa_ticketer.tz

.PHONY: build-kernel
build-kernel:
	@cargo build --package jstz_kernel --target wasm32-unknown-unknown $(PROFILE_OPT)

.PHONY: build-jstzd-kernel
build-jstzd-kernel: build-kernel
	@cp target/wasm32-unknown-unknown/$(PROFILE_TARGET_DIR)/jstz_kernel.wasm $(JSTZD_KERNEL_PATH)

.PHONY: build-cli
build-cli:
	@cargo build --package jstz_cli $(PROFILE_OPT)

.PHONY: build-deps
build-deps:
	@rustup target add wasm32-unknown-unknown

.PHONY: build-dev-deps
build-dev-deps: build-deps
	@rustup component add rustfmt clippy

.PHONY: build-sdk-wasm-pkg
build-sdk-wasm-pkg:
	@cd crates/jstz_sdk && wasm-pack build --target bundler --release

.PHONY: build-native-kernel
build-native-kernel:
	@cargo build -p jstz_engine --release --features "native-kernel"

.PHONY: riscv-runtime
riscv-runtime:
	@RUSTY_V8_ARCHIVE=$$RISCV_V8_ARCHIVE_DIR/librusty_v8.a RUSTY_V8_SRC_BINDING_PATH=$$RISCV_V8_ARCHIVE_DIR/src_binding.rs cargo build -p jstz_runtime --release --target riscv64gc-unknown-linux-musl

.PHONY: riscv-v2-one-shot-kernel
riscv-v2-one-shot-kernel:
	@RUSTY_V8_ARCHIVE=$$RISCV_V8_ARCHIVE_DIR/librusty_v8.a RUSTY_V8_SRC_BINDING_PATH=$$RISCV_V8_ARCHIVE_DIR/src_binding.rs cargo build -p jstz_kernel --no-default-features --features v2_runtime --release --target riscv64gc-unknown-linux-musl

.PHONY: riscv-pvm-kernel
riscv-pvm-kernel:
	@unset NIX_LDFLAGS && RUSTY_V8_ARCHIVE=$$RISCV_V8_ARCHIVE_DIR/librusty_v8.a \
		RUSTY_V8_SRC_BINDING_PATH=$$RISCV_V8_ARCHIVE_DIR/src_binding.rs \
		cargo build \
		-p jstz_kernel \
		--no-default-features \
		--features riscv_kernel \
		--release \
		--target riscv64gc-unknown-linux-musl

.PHONY: test
test: test-unit test-int

.PHONY: test-v2
test-v2: test-unit-v2 test-int-v2

.PHONY: test-unit
test-unit:
# --lib only runs unit tests in library crates
# --bins only runs unit tests in binary crates
	@cargo nextest run --lib --bins --workspace --exclude "jstz_tps_bench" --features skip-wpt,skip-rollup-tests --config-file .config/nextest.toml

.PHONY: test-int
test-int:
# --test only runs a specified integration test (a test in /tests).
#        the glob pattern is used to match all integration tests
# --exclude excludes the jstz_api wpt test
	@cargo nextest run --test "*" --workspace --exclude "jstz_api" --features skip-wpt,skip-rollup-tests

test-unit-v2:
# --lib only runs unit tests in library crates
# --bins only runs unit tests in binary crates
	@cargo nextest run --lib --bins --workspace --exclude "jstz_tps_bench" --features v2_runtime,skip-wpt,skip-rollup-tests --config-file .config/nextest.toml

.PHONY: test-int
test-int-v2:
# --test only runs a specified integration test (a test in /tests).
#        the glob pattern is used to match all integration tests
# --exclude excludes the jstz_api wpt test
	@cargo nextest run --test "*" --workspace --exclude "jstz_api" --features v2_runtime,skip-wpt,skip-rollup-tests

.PHONY: cov
cov:
	@cargo llvm-cov --workspace --exclude-from-test "jstz_api" --html --open

.PHONY: check
check: lint fmt

.PHONY: clean
clean:
	@cargo clean
	@rm -f result
	@rm -rf logs

.PHONY: fmt
fmt:
	nix fmt

.PHONY: fmt-check
fmt-check:
	nix fmt -- --fail-on-change

.PHONY: lint
lint:
#  Jstzd has to processes a non-empty kernel in its build script
	@echo "ignore" > $(JSTZD_KERNEL_PATH)
	@cargo clippy --all-targets --features skip-wpt -- --deny warnings
	@rm -f $(JSTZD_KERNEL_PATH)

.PHONY: run-manual-test
run-manual-test: riscv-pvm-kernel
	@riscv-sandbox run --timings --address sr1FXevDx86EyU1BBwhn94gtKvVPTNwoVxUC --inbox-file manual_test/inbox.json --input target/riscv64gc-unknown-linux-musl/release/kernel-executable
