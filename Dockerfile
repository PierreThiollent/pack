# syntax=docker/dockerfile:1

ARG ALPINE_VERSION=3.23
ARG RUST_VERSION=1.96

FROM rust:${RUST_VERSION}-alpine${ALPINE_VERSION} AS builder

ARG PACK_VERSION

RUN apk add --no-cache \
    build-base \
    libssh2-dev \
    libssh2-static \
    openssl-dev \
    openssl-libs-static \
    pkgconf \
    zlib-static

WORKDIR /usr/src/pack

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --locked \
    && strip target/release/pack \
    && if [ -n "${PACK_VERSION}" ]; then \
        test "$(target/release/pack --version)" = "pack ${PACK_VERSION#v}"; \
    fi

FROM alpine:${ALPINE_VERSION} AS runtime

RUN apk add --no-cache \
    ca-certificates \
    libssh2 \
    mariadb-client \
    openssl \
    postgresql-client \
    tzdata \
    && addgroup --system --gid 10001 pack \
    && adduser --system --disabled-password --uid 10001 --ingroup pack --home /home/pack pack \
    && mkdir -p /backups /etc/pack /home/pack/.pack /source \
    && chown -R pack:pack /backups /home/pack

COPY --from=builder /usr/src/pack/target/release/pack /usr/local/bin/pack

ENV HOME=/home/pack

WORKDIR /home/pack

USER 10001:10001

ENTRYPOINT ["/usr/local/bin/pack"]
CMD ["run", "--config", "/etc/pack/pack.yml"]
