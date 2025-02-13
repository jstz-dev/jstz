#!/bin/sh
git clone https://github.com/servo/mozjs.git
cd mozjs && cat mozjs-sys/mozjs/build/autoconf/config.sub | sed 's/riscv64 | riscv64be/riscv64 | riscv64be | riscv64gc/g' > a && mv a mozjs-sys/mozjs/build/autoconf/config.sub

rustup target add --toolchain 1.82.0 riscv64gc-unknown-linux-gnu
env MOZJS_FROM_SOURCE=1 cargo +1.82.0 build --target=riscv64gc-unknown-linux-gnu  --verbose
