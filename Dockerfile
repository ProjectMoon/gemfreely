# This dockerfile is used for the CI pipeline, pre-installed build
# deps, so we don't spend time compiling them during every CI run.
FROM rust:1.76-slim
RUN rustup component add rustfmt
RUN cargo install --locked cargo-deny
RUN apt update && apt install -y pkg-config
RUN apt install -y libssl-dev
