#!/bin/sh

set -eu

image_name="${1:-pack:smoke-test}"
temporary_directory="$(mktemp -d)"
container_id=""

cleanup() {
  if [ -n "${container_id}" ]; then
    docker rm --force "${container_id}" >/dev/null 2>&1 || true
  fi
  rm -rf "${temporary_directory}"
}

trap cleanup EXIT INT TERM

cat > "${temporary_directory}/pack.yml" <<'EOF'
models: {}
EOF

echo "Building ${image_name}"
docker build --tag "${image_name}" .

echo "Checking Pack and database clients"
docker run --rm "${image_name}" --version
docker run --rm --entrypoint mysqldump "${image_name}" --version
docker run --rm --entrypoint pg_dump "${image_name}" --version

echo "Checking the non-root runtime contract"
docker run --rm --entrypoint /bin/sh "${image_name}" -c '
  set -eu
  test "$(id -u)" = "10001"
  test "$(id -g)" = "10001"
  test "${HOME}" = "/home/pack"
  test -r /usr/local/bin/pack
  test -w /backups
  test -w /home/pack/.pack
'

echo "Checking the default command and graceful shutdown"
container_id="$(docker create --volume "${temporary_directory}/pack.yml:/etc/pack/pack.yml:ro" "${image_name}")"
docker start "${container_id}" >/dev/null

attempt=0
until docker logs "${container_id}" 2>&1 | grep -q "Scheduler started"; do
  attempt=$((attempt + 1))
  if [ "${attempt}" -ge 30 ]; then
    docker logs "${container_id}" >&2
    echo "Pack scheduler did not start" >&2
    exit 1
  fi
  sleep 1
done

docker stop --time 10 "${container_id}" >/dev/null
exit_code="$(docker inspect --format '{{.State.ExitCode}}' "${container_id}")"

if [ "${exit_code}" -ne 0 ]; then
  docker logs "${container_id}" >&2
  echo "Pack exited with code ${exit_code} after Docker stop" >&2
  exit 1
fi

echo "Docker smoke test passed"
