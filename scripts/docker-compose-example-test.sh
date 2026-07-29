#!/bin/sh

set -eu

image_name="${1:-pack:compose-example-test}"
compose_file="examples/docker-compose/compose.yaml"
project_name="pack-example-test-$$"

export PACK_IMAGE="${image_name}"
export POSTGRES_DB="pack_example"
export POSTGRES_USER="pack_example"
export POSTGRES_PASSWORD="pack_example_password"

compose() {
  docker compose --project-name "${project_name}" --file "${compose_file}" "$@"
}

cleanup() {
  exit_code=$?
  trap - EXIT INT TERM

  if [ "${exit_code}" -ne 0 ]; then
    compose logs >&2 || true
  fi

  compose down --volumes --remove-orphans >/dev/null 2>&1 || true
  exit "${exit_code}"
}

trap cleanup EXIT INT TERM

if [ "${PACK_DOCKER_SKIP_BUILD:-0}" = "1" ]; then
  echo "Using pre-built image ${image_name}"
else
  echo "Building ${image_name}"
  docker build --tag "${image_name}" .
fi

echo "Starting the example application and database"
compose up --detach --wait application database

echo "Running the Pack pipeline"
compose run --rm --no-deps pack perform --config /etc/pack/pack.yml

echo "Checking the database dump, archived file, and cycler state"
# shellcheck disable=SC2016 # Variables expand inside the container shell.
compose run --rm --no-deps --entrypoint /bin/sh pack -c '
  set -eu

  set -- /backups/application-*.tar.gz
  test "$#" -eq 1
  artifact_path="$1"
  test -f "${artifact_path}"

  database_dump_path="application/${POSTGRES_DB}.sql"
  tar -xOzf "${artifact_path}" "${database_dump_path}" \
    | grep -q "pack_database_marker"

  tar -xOzf "${artifact_path}" application/archive.tar > /tmp/application-files.tar
  tar -xOf /tmp/application-files.tar archive/source/application/uploads/example-document.txt \
    | grep -q "pack_application_marker"

  state_path=/home/pack/.pack/cycler/application_local.json
  test -f "${state_path}"
  grep -q "$(basename "${artifact_path}")" "${state_path}"
'

echo "Docker Compose example test passed"
