FROM rust:1-alpine3.23 AS builder
ENV RUSTFLAGS="-C target-feature=-crt-static"
RUN apk add --no-cache musl-dev \
    # Required for git-version
    git

WORKDIR /verdantgolem
COPY . /verdantgolem

RUN rustup show active-toolchain || rustup toolchain install
RUN rustup component add rustfmt

# build release
RUN --mount=type=cache,sharing=private,target=/verdantgolem/target \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --release -p verdantgolem && cp target/release/verdantgolem ./verdantgolem.release

FROM alpine:3.24

COPY --from=builder /verdantgolem/verdantgolem.release /bin/verdantgolem

# set workdir to /verdantgolem, this is required to influence the PWD environment variable
# it allows for bind mounting the server files without overwriting the pumpkin
# executable (without requiring an `docker cp`-ing the binary to the host folder)
WORKDIR /verdantgolem

RUN apk add --no-cache libgcc && chown 2613:2613 .

ENV RUST_BACKTRACE=1
EXPOSE 25565
USER 2613:2613
ENTRYPOINT [ "/bin/verdantgolem" ]
HEALTHCHECK CMD nc -z 127.0.0.1 25565 || exit 1
