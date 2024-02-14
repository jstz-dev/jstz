.PHONY: all
all: test build check

.PHONY: build
build: build-cli-kernel
	@cargo build --release

.PHONY: build-bridge
build-bridge:
	@ligo compile contract contracts/jstz_bridge.mligo \
		--module "Jstz_bridge" > contracts/jstz_bridge.tz

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

.PHONY: test
test:
	@cargo test

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

# FIXME: 
# Clippy builds the CLI since the CLI must expand the macro containing the bytes of the `jstz_kernel.wasm` file. 
# So in order to lint the CLI we need to build the kernel
.PHONY: lint
lint: build-cli-kernel
	@cargo clippy -- --no-deps -D warnings -A clippy::let_underscore_future -A clippy::module_inception -A clippy::op_ref -A clippy::manual_strip -A clippy::missing_safety_doc -A clippy::slow_vector_initialization -A clippy::empty_loop -A clippy::collapsible-match