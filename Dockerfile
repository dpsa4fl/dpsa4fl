FROM rust:1.69.0-alpine AS chef
RUN apk add --no-cache libc-dev
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
RUN cargo install cargo-chef --version 0.1.60 && \
    rm -r $CARGO_HOME/registry
WORKDIR /src

FROM chef AS planner
COPY Cargo.toml /src/Cargo.toml
COPY Cargo.lock /src/Cargo.lock
COPY src /src/src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /src/recipe.json /src/recipe.json
RUN cargo chef cook --release
COPY Cargo.toml /src/Cargo.toml
COPY Cargo.lock /src/Cargo.lock
COPY src /src/src
ARG BINARY=dpsa4fl-janus-manager
ARG GIT_REVISION=unknown
ENV GIT_REVISION ${GIT_REVISION}
RUN cargo build --release

FROM alpine:3.18.0 AS final
ARG BINARY=dpsa4fl-janus-manager
ARG GIT_REVISION=unknown
LABEL revision ${GIT_REVISION}
COPY --from=builder /src/target/release/$BINARY /$BINARY
# Store the build argument in an environment variable so we can reference it
# from the ENTRYPOINT at runtime.
ENV BINARY=$BINARY
ENTRYPOINT ["/bin/sh", "-c", "exec /$BINARY \"$0\" \"$@\""]
