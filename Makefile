.PHONY: all
all: build test check

.PHONY: build
build:
	@cargo build --target wasm32-unknown-unknown --release

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
check:
	@cargo fmt --check
	@cargo clippy --all-targets -- --deny warnings

.PHONY: clean
clean:
	@cargo clean