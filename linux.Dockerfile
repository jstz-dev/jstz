FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive

# install core tools
RUN apt-get update && apt-get install -y \
    curl git direnv openssh-server sudo ca-certificates xz-utils \
  && rm -rf /var/lib/apt/lists/* \
  && mkdir /var/run/sshd

# add a non-root user
RUN useradd -m devuser \
  && echo 'devuser ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers

# install Nix as devuser
USER devuser
WORKDIR /home/devuser
RUN curl -L https://nixos.org/nix/install | sh

# hook nix and direnv into bash
RUN echo 'source ~/.nix-profile/etc/profile.d/nix.sh' >> ~/.bashrc \
  && echo 'eval "$(direnv hook bash)"'      >> ~/.bashrc

USER root
# prepare SSH for devuser
RUN mkdir -p /home/devuser/.ssh \
  && chown devuser:devuser /home/devuser/.ssh

RUN useradd -m -s /bin/bash devuser \
  && echo 'devuser ALL=(ALL) NOPASSWD:ALL' >> /etc/sudoers

EXPOSE 22
CMD ["/usr/sbin/sshd","-D"]
