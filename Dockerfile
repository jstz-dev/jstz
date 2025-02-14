FROM ubuntu:24.04

# add arch
RUN sed 's/^deb http/deb [arch=arm64] http/' -i '/etc/apt/sources.list'
RUN echo 'deb [arch=riscv64] http://ports.ubuntu.com/ubuntu-ports noble main universe multiverse restricted' >> /etc/apt/sources.list && \
    echo 'deb [arch=riscv64] http://ports.ubuntu.com/ubuntu-ports noble-updates main universe multiverse restricted' >> /etc/apt/sources.list && \
    echo 'deb [arch=riscv64] http://ports.ubuntu.com/ubuntu-ports noble-backports main universe multiverse restricted' >> /etc/apt/sources.list && \
    echo 'deb [arch=riscv64] http://ports.ubuntu.com/ubuntu-ports noble-security main universe multiverse restricted' >> /etc/apt/sources.list
RUN dpkg --add-architecture riscv64
# add for actions
RUN apt update && apt install -y curl git && apt-get clean
# add main build deps
RUN apt install --no-install-recommends -y build-essential pkg-config m4 python3 python3-setuptools llvm llvm-dev lld libclang-dev clang && apt-get clean
# add cross deps
RUN apt install --no-install-recommends -y gcc-riscv64-linux-gnu g++-riscv64-linux-gnu qemu-user qemu-user-static && apt-get clean
# add runtime deps
RUN apt install --no-install-recommends -y libc6:riscv64 libstdc++6:riscv64 && apt-get clean

RUN sh -c 'for v in $(ls "/usr/bin" | grep "riscv64-linux-gnu"); do t=$(echo "$v" | sed -e "s/riscv64-linux-gnu/riscv64gc-unknown-linux-gnu/g"); ln -s "/usr/bin/$v" "/usr/bin/$t" ; done'
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && . "$HOME/.cargo/env" && rustup toolchain install 1.82.0 && rustup target add --toolchain 1.82.0 riscv64gc-unknown-linux-musl
