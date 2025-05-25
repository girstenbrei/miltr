
FROM rust:1.87 AS chef
WORKDIR /workspace

# Install additional tooling
RUN cargo install \
    cargo-chef \
    cargo-tarpaulin

# Install system dependencies
RUN apt-get update \
 && apt-get install -y postfix swaks

# Setup postfix
COPY ./server/tests/postfix/config /etc/postfix
RUN echo "localhost" > /etc/mailname
RUN cd /etc/postfix && postmap /etc/postfix/transport


FROM chef AS planner

COPY . .
RUN cargo chef prepare --recipe-path recipe.json


FROM chef AS builder
COPY --from=planner /workspace/recipe.json recipe.json

RUN cargo chef cook --recipe-path recipe.json --tests
COPY . .
