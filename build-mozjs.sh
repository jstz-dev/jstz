#!/bin/sh
apt install -y wget
dst="/tmp/riscv64-linux-musl-cross"
wget -O /tmp/riscv64-linux-musl-cross.tgz https://musl.cc/riscv64-linux-musl-cross.tgz && tar -xf /tmp/riscv64-linux-musl-cross.tgz -C /tmp
for v in $(ls "$dst/bin" | grep "riscv64-linux"); do t=$(echo "$v" | sed -e "s/riscv64-linux-musl/riscv64gc-unknown-linux-musl/g"); ln -s "$dst/bin/$v" "$dst/bin/$t" ; done
export PATH=$PATH:$dst/bin
git clone https://github.com/servo/mozjs.git --branch mozjs-sys-v0.128.6-1 --single-branch
cd mozjs && cat mozjs-sys/mozjs/build/autoconf/config.sub | sed 's/riscv64 | riscv64be/riscv64 | riscv64be | riscv64gc/g' > a && mv a mozjs-sys/mozjs/build/autoconf/config.sub
rustup target add --toolchain 1.82.0 riscv64gc-unknown-linux-musl
env MOZJS_CREATE_ARCHIVE=1 BINDGEN_EXTRA_CLANG_ARGS='-target riscv64-unknown-linux-musl -I/usr/include/riscv64-linux-musl --sysroot=/ -Wno-unused-command-line-argument -fuse-ld=lld' CLANGFLAGS="-target riscv64-unknown-linux-musl -I/usr/include/riscv64-linux-musl --sysroot=/ -Wno-unused-command-line-argument -fuse-ld=lld" CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_RUNNER="/usr/bin/qemu-riscv64-static" CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_LINKER="/tmp/riscv64-linux-musl-cross/bin/riscv64-linux-musl-gcc" CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_CC="/tmp/riscv64-linux-musl-cross/bin/riscv64-linux-musl-gcc" CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_CXX="/tmp/riscv64-linux-musl-cross/bin/riscv64-linux-musl-g++" CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_AR="/tmp/riscv64-linux-musl-cross/bin/riscv64-linux-musl-ar" MOZJS_FROM_SOURCE=1 cargo +1.82.0 build --target=riscv64gc-unknown-linux-musl
