# Cloud Agent base image for the punktfunk Rust workspace.
#
# Ubuntu 26.04 to match the CI builder (ci/rust-ci.Dockerfile): the workspace links real
# system libs, and both libsdl3-dev and gtk4 >= 4.16 (gdk4-sys) ship only on 26.04, not 24.04.
# The system-lib list below tracks ci/rust-ci.Dockerfile, the authoritative set — keep them in
# sync when a link dependency changes.
FROM ubuntu:26.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential clang libclang-dev pkg-config cmake git curl ca-certificates unzip sudo \
      mold \
      libpipewire-0.3-dev libopus-dev libwayland-dev libxkbcommon-dev \
      libgl-dev libegl-dev libgbm-dev \
      libgtk-4-dev libadwaita-1-dev libsdl3-dev \
    && rm -rf /var/lib/apt/lists/*

# The agent runs as `ubuntu` (uid 1000 on the cloud image). Give it passwordless sudo so setup
# can install extra packages without a rebuild.
RUN id ubuntu >/dev/null 2>&1 || useradd -m -u 1000 -s /bin/bash ubuntu \
    && install -d -m0755 /etc/sudoers.d \
    && echo 'ubuntu ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/90-ubuntu \
    && chmod 0440 /etc/sudoers.d/90-ubuntu

# Rust toolchain shared under /usr/local so the ubuntu user can use and extend it. The exact
# channel is pinned by rust-toolchain.toml; install it here so the first cargo run needs no
# network. mold links the x86_64 targets (see ci/cargo-config-mold.toml).
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --no-modify-path --profile minimal \
          --default-toolchain 1.96.0 --component rustfmt clippy \
    && printf '[target.x86_64-unknown-linux-gnu]\nrustflags = ["-C", "link-arg=-fuse-ld=mold"]\n' \
      > "$CARGO_HOME/config.toml" \
    && chmod -R a+w "$RUSTUP_HOME" "$CARGO_HOME" \
    && rustc --version && cargo clippy --version && cargo fmt --version && mold --version

# bun builds the web console, docs site and the SDK/plugin-kit TypeScript. Pinned release asset
# checked by SHA-256, the same version and sum the CI image uses (bump both together).
ARG BUN_VERSION=1.3.14
ARG BUN_SHA=a063908ae08b7852ca10939bbdc6ceed3ddabce8fb9402dce83d65d73b36e6c7
RUN curl -fsSL -o /tmp/bun.zip \
      "https://github.com/oven-sh/bun/releases/download/bun-v${BUN_VERSION}/bun-linux-x64-baseline.zip" \
    && echo "${BUN_SHA}  /tmp/bun.zip" | sha256sum -c - \
    && unzip -q -o -j /tmp/bun.zip '*/bun' -d /tmp \
    && install -m0755 /tmp/bun /usr/local/bin/bun \
    && rm -f /tmp/bun.zip /tmp/bun \
    && bun --version

USER ubuntu
