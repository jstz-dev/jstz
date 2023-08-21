.PHONY: all
all: build test check

.PHONY: build-installer
build-installer: build
	@smart-rollup-installer get-reveal-installer \
		--upgrade-to target/wasm32-unknown-unknown/release/jstz_kernel.wasm \
		--output target/kernel/jstz_kernel_installer.hex \
		--preimages-dir target/kernel/preimages/

.PHONY: build-bridge
build-bridge:
	@ligo compile contract jstz_bridge/jstz_bridge.mligo \
		--module "Jstz_bridge" > jstz_bridge/jstz_bridge.tz

.PHONY: build
build:
	@cargo build --target wasm32-unknown-unknown --release

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

.PHONY: fmt-nix
fmt-nix:
	@alejandra ./

.PHONY: fmt-rust
fmt-rust:
	@cargo fmt

.PHONY: fmt
fmt: fmt-nix fmt-rust
