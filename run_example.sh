#!/bin/sh

env RUSTFLAGS='-latomic' PATH=$PATH:/tmp/riscv64-linux-musl-cross/bin CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_RUNNER="/usr/bin/qemu-riscv64-static" CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_LINKER="riscv64-linux-musl-gcc" CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_AR="riscv64-linux-musl-ar" MOZJS_ARCHIVE=/mozjs/target/libmozjs-riscv64gc-unknown-linux-musl.tar.gz cargo +1.82.0 build -p jstz_engine --release --features "native-kernel" --target riscv64gc-unknown-linux-musl
QEMU_LD_PREFIX=/tmp/riscv64-linux-musl-cross/riscv64-linux-musl/ ./target/riscv64gc-unknown-linux-musl/release/jstz_engine
