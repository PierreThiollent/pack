# syntax=docker/dockerfile:1

ARG ALPINE_VERSION=3.23
ARG UBUNTU_VERSION=24.04
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

FROM ubuntu:${UBUNTU_VERSION} AS runtime-base

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        default-mysql-client \
        gzip \
        libssh2-1 \
        openssl \
        postgresql-client \
        tar \
        tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 pack \
    && useradd --system --uid 10001 --gid 10001 --home-dir /home/pack --create-home --shell /usr/sbin/nologin pack \
    && mkdir -p /backups /etc/pack /home/pack/.pack /source \
    && chown -R pack:pack /backups /home/pack

ENV HOME=/home/pack

WORKDIR /home/pack

USER 10001:10001

ENTRYPOINT ["/usr/local/bin/pack"]
CMD ["run", "--config", "/etc/pack/pack.yml"]

FROM runtime-base AS runtime-from-source

COPY --from=builder /usr/src/pack/target/release/pack /usr/local/bin/pack

FROM runtime-base AS runtime-from-release

ARG PACK_REPOSITORY=pierrethiollent/pack
ARG PACK_VERSION
ARG TARGETARCH

USER 0:0

RUN set -eux; \
    test -n "${PACK_VERSION}"; \
    case "${TARGETARCH}" in \
        amd64|arm64) package_arch="${TARGETARCH}" ;; \
        *) echo "Unsupported architecture: ${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    apt-get update; \
    apt-get install --yes --no-install-recommends curl gzip tar; \
    curl --fail --location --silent --show-error \
        "https://github.com/${PACK_REPOSITORY}/releases/download/${PACK_VERSION}/pack-linux-${package_arch}.tar.gz" \
        | tar --extract --gzip --directory /usr/local/bin pack; \
    chmod 0755 /usr/local/bin/pack; \
    apt-get purge --yes --auto-remove curl; \
    rm -rf /var/lib/apt/lists/*; \
    /usr/local/bin/pack --version | grep -Fx "pack ${PACK_VERSION#v}"

USER 10001:10001

FROM runtime-from-source AS runtime
