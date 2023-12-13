.PHONY: all
all: build test check

.PHONY: build-installer
build-installer: build
	@rm -rf target/kernel/preimages
	@smart-rollup-installer get-reveal-installer \
		--upgrade-to target/wasm32-unknown-unknown/release/jstz_kernel.wasm \
		--output target/kernel/jstz_kernel_installer.hex \
		--preimages-dir target/kernel/preimages/

.PHONY: build-bridge
build-bridge:
	@ligo compile contract contracts/jstz_bridge.mligo \
		--module "Jstz_bridge" > contracts/jstz_bridge.tz

.PHONY: build
build:
	@cargo build --package jstz_kernel --target wasm32-unknown-unknown --release

.PHONY: build-deps
build-deps:
	@rustup target add wasm32-unknown-unknown
	@cargo install tezos-smart-rollup-installer

.PHONY: build-dev-deps
build-dev-deps: build-deps
	@rustup component add rustfmt clippy

.PHONY: test
test:
	@cargo test

.PHONY: check
check:
	@cargo fmt --check
	@cargo clippy --all-targets -- --deny warnings

.PHONY: clean
clean:
	@cargo clean
	rm -f result
	rm -rf logs

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
	@cargo clippy -- -D warnings -A clippy::let_underscore_future -A clippy::module_inception -A clippy::op_ref -A clippy::manual_strip -A clippy::missing_safety_doc -A clippy::slow_vector_initialization -A clippy::empty_loop