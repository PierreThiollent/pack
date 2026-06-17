#!/usr/bin/env sh

# pack installer
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/PierreThiollent/pack/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/PierreThiollent/pack/main/install.sh | sh -s v0.1.0

set -eu

repo="PierreThiollent/pack"
bin="pack"
version="latest"
install_dir="/usr/local/bin"

say() {
  printf '%s\n' "$*"
}

err() {
  printf 'error: %s\n' "$*" >&2
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    err "$1 is required but was not found"
    exit 1
  fi
}

resolve_latest_version() {
  curl -fsSL "https://api.github.com/repos/${repo}/releases/latest" |
    awk 'BEGIN{FS=": |,|\""}; /tag_name/{print $5}'
}

detect_platform() {
  case "$(uname -s)" in
    Linux)
      printf 'linux'
      ;;
    Darwin)
      printf 'darwin'
      ;;
    *)
      err "unsupported operating system: $(uname -s)"
      exit 1
      ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64 | amd64)
      printf 'amd64'
      ;;
    arm64 | aarch64)
      printf 'arm64'
      ;;
    *)
      err "unsupported architecture: $(uname -m)"
      exit 1
      ;;
  esac
}

install_binary() {
  source_path="$1"
  target_path="${install_dir}/${bin}"

  mkdir -p "${install_dir}"

  if [ "$(id -u)" -eq 0 ] || [ -w "${install_dir}" ]; then
    mv "${source_path}" "${target_path}"
  else
    need_cmd sudo
    sudo mv "${source_path}" "${target_path}"
  fi
}

main() {
  need_cmd curl
  need_cmd tar
  need_cmd uname
  need_cmd mktemp

  if [ $# -eq 1 ] && [ "$1" != "latest" ]; then
    version="$1"
  fi

  if [ "${version}" = "latest" ]; then
    version="$(resolve_latest_version)"
  fi

  if [ -z "${version}" ]; then
    err "failed to resolve latest ${bin} release"
    exit 1
  fi

  platform="$(detect_platform)"
  arch="$(detect_arch)"
  package="${bin}-${platform}-${arch}.tar.gz"
  package_url="https://github.com/${repo}/releases/download/${version}/${package}"
  tmp_dir="$(mktemp -d)"
  archive_path="${tmp_dir}/${package}"
  extracted_bin="${tmp_dir}/${bin}"
  target_path="${install_dir}/${bin}"

  trap 'rm -rf "${tmp_dir}"' EXIT

  say "pack installer"
  say "  version:     ${version}"
  say "  platform:    ${platform}"
  say "  arch:        ${arch}"
  say "  install dir: ${install_dir}"

  if [ -x "${target_path}" ]; then
    current_version="v$(${target_path} --version | awk '{print $NF}')"
    if [ "${current_version}" = "${version}" ]; then
      say "pack is already up to date (${version})."
      exit 0
    fi

    say "upgrading from ${current_version} to ${version}"
  fi

  say "downloading ${package}"
  if ! curl -fL "${package_url}" -o "${archive_path}"; then
    err "failed to download ${package_url}"
    exit 1
  fi

  tar -xzf "${archive_path}" -C "${tmp_dir}"

  if [ ! -f "${extracted_bin}" ]; then
    err "release archive does not contain ${bin}"
    exit 1
  fi

  chmod +x "${extracted_bin}"
  install_binary "${extracted_bin}"
  mkdir -p "${HOME}/.pack"

  say "pack ${version} has been installed to ${target_path}."
}

main "$@"
