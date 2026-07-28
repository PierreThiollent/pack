#!/bin/sh

set -eu

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  echo "Usage: $0 <git-tag> [image]" >&2
  exit 1
fi

git_tag="$1"
image_name="${2:-}"

if ! printf '%s\n' "${git_tag}" | grep -Eq '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'; then
  echo "Release tag must start with v and contain a semantic version: ${git_tag}" >&2
  exit 1
fi

release_version="${git_tag#v}"
package_version="$(awk '
  $0 == "[package]" {
    in_package = 1
    next
  }
  in_package && /^\[/ {
    exit
  }
  in_package && /^version = "/ {
    gsub(/^version = "/, "")
    gsub(/"$/, "")
    print
    exit
  }
' Cargo.toml)"

if [ -z "${package_version}" ]; then
  echo "Failed to read the package version from Cargo.toml" >&2
  exit 1
fi

if [ "${package_version}" != "${release_version}" ]; then
  echo "Release tag ${git_tag} does not match Cargo.toml version ${package_version}" >&2
  exit 1
fi

if [ -n "${image_name}" ]; then
  expected_output="pack ${release_version}"
  actual_output="$(docker run --rm "${image_name}" --version)"

  if [ "${actual_output}" != "${expected_output}" ]; then
    echo "Image version mismatch: expected '${expected_output}', got '${actual_output}'" >&2
    exit 1
  fi
fi

echo "Release version ${release_version} is consistent"
