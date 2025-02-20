FROM ubuntu:24.04

# add arch
RUN sed 's/^deb http/deb [arch=arm64] http/' -i '/etc/apt/sources.list'
RUN echo 'deb [arch=riscv64] http://ports.ubuntu.com/ubuntu-ports noble main universe multiverse restricted' >> /etc/apt/sources.list && \
    echo 'deb [arch=riscv64] http://ports.ubuntu.com/ubuntu-ports noble-updates main universe multiverse restricted' >> /etc/apt/sources.list && \
    echo 'deb [arch=riscv64] http://ports.ubuntu.com/ubuntu-ports noble-backports main universe multiverse restricted' >> /etc/apt/sources.list && \
    echo 'deb [arch=riscv64] http://ports.ubuntu.com/ubuntu-ports noble-security main universe multiverse restricted' >> /etc/apt/sources.list
RUN dpkg --add-architecture riscv64
# add for actions
RUN apt update && apt install -y curl git wget && apt-get clean
# add main build deps
RUN apt install --no-install-recommends -y build-essential pkg-config m4 python3 python3-setuptools llvm llvm-dev lld libclang-dev clang && apt-get clean
# add cross deps
RUN apt install --no-install-recommends -y gcc-riscv64-linux-gnu g++-riscv64-linux-gnu qemu-user qemu-user-static && apt-get clean
# add runtime deps
RUN apt install --no-install-recommends -y libc6:riscv64 libstdc++6:riscv64 && apt-get clean
RUN find "/usr/bin/" -name "riscv64-linux-gnu*" -exec sh -c 't=$(basename $0 | sed -e "s/riscv64-linux-gnu/riscv64gc-unknown-linux-gnu/g"); ln -s "/usr/bin/$(basename $0)" "/usr/bin/$t"' {} \;

ENV PATH="$PATH:/tmp/riscv64-linux-musl-cross/bin"

RUN dst="/tmp/riscv64-linux-musl-cross" && wget -O /tmp/riscv64-linux-musl-cross.tgz https://musl.cc/riscv64-linux-musl-cross.tgz && tar -xf /tmp/riscv64-linux-musl-cross.tgz -C /tmp && rm https://musl.cc/riscv64-linux-musl-cross.tgz && find "$dst/bin/" -name "riscv64-linux*" -exec sh -c 't=$(basename $1 | sed -e "s/riscv64-linux-musl/riscv64gc-unknown-linux-musl/g"); ln -s "$0/$(basename $1)" "$0/$t"' "$dst/bin" {} \; && ln -s $dst/riscv64-linux-musl/lib/libc.so /lib/ld-musl-riscv64.so.1
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && . "$HOME/.cargo/env" && rustup toolchain install 1.82.0 && rustup target add --toolchain 1.82.0 riscv64gc-unknown-linux-musl
