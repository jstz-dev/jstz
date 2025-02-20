#!/bin/sh

git clone https://github.com/servo/mozjs.git --branch mozjs-sys-v0.128.3-0 --single-branch
cd mozjs && cat mozjs-sys/mozjs/build/autoconf/config.sub | sed 's/riscv64 | riscv64be/riscv64 | riscv64be | riscv64gc/g' > a && mv a mozjs-sys/mozjs/build/autoconf/config.sub

env MOZJS_CREATE_ARCHIVE=1 CLANGFLAGS="-target riscv64-unknown-linux-musl --sysroot=/ -Wno-unused-command-line-argument -fuse-ld=lld" BINDGEN_EXTRA_CLANG_ARGS='-target riscv64-unknown-linux-musl --sysroot=/ -Wno-unused-command-line-argument -fuse-ld=lld' CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_RUNNER="/usr/bin/qemu-riscv64-static" CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_LINKER="riscv64-linux-musl-gcc" CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_AR="riscv64-linux-musl-ar" MOZJS_FROM_SOURCE=1 cargo +1.82.0 build --target=riscv64gc-unknown-linux-musl
