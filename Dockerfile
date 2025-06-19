
FROM rust:1.87 AS chef
WORKDIR /workspace

# Install additional tooling
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall --target x86_64-unknown-linux-musl \
    cargo-chef \
    cargo-tarpaulin \
    cargo-nextest

# Install system dependencies
RUN apt-get update \
 && apt-get install -y postfix swaks vim

# Setup postfix
COPY ./server/tests/postfix/config /etc/postfix
RUN echo "localhost" > /etc/mailname \
 && chmod 644 /etc/postfix/*


FROM chef AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json


FROM chef AS builder
COPY --from=planner /workspace/recipe.json recipe.json

RUN cargo chef cook --recipe-path recipe.json --tests
COPY . .
