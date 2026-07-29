# Docker Compose example

This example shows how Pack can back up the following resources in a single model:

- a PostgreSQL database running in a `database` container;
- a MySQL 8.4 database running in a `mysql` container;
- persistent files produced by an `application` container;
- the final artifact in a separate local volume;
- Pack's retention state in another persistent volume.

The `application`, `database`, and `mysql` containers are intentionally simple. They represent services from an existing project without requiring a specific framework.

## Architecture

```text
application ── application_data volume ──(read-only)── Pack
database ───── Compose network ─────────────────────── Pack
mysql ──────── Compose network ─────────────────────── Pack
                                                     ├── pack_backups
                                                     └── pack_state
```

Pack connects to PostgreSQL through the `database` service name and to MySQL through the `mysql` service name. Inside a container, `localhost` refers to that container itself and cannot be used to reach another service.

## Start the example

Copy the environment variables and replace the example password:

```bash
cd examples/docker-compose
cp .env.example .env
$EDITOR .env
```

Start all three services:

```bash
docker compose up -d
```

Pack runs its scheduler with:

```text
pack run --config /etc/pack/pack.yml
```

Follow its logs:

```bash
docker compose logs -f pack
```

To run a backup immediately without waiting for the schedule:

```bash
docker compose run --rm pack perform --config /etc/pack/pack.yml
```

> [!WARNING]
> This command starts a second Pack container. Running `perform` while the scheduler container is already executing a backup can cause concurrent writes to `/backups` and the shared cycler state in `/home/pack/.pack`. Stop the `pack` service first, or make sure no scheduled backup can run at the same time.

List the local artifacts:

```bash
docker compose run --rm --entrypoint /bin/sh pack -c 'ls -lh /backups'
```

`docker compose down` removes the containers and network but preserves the volumes. Only use `docker compose down --volumes` if you accept deleting the database, application files, Pack state, and local backups created by this example.

## Adapt it to an existing application

The `application` service only creates a marker document in `application_data`. In your project, share the volume that contains non-reproducible data with Pack, such as uploads, generated documents, media, or other user files.

The application service keeps write access:

```yaml
services:
  application:
    volumes:
      - application_data:/application/write/path
```

Pack mounts the same volume as read-only:

```yaml
services:
  pack:
    volumes:
      - application_data:/source/application:ro
```

Then update `archive.includes` in `pack.yml`. A bind mount can also be used when a directory lives directly on the host:

```yaml
services:
  pack:
    volumes:
      - ./var/uploads:/source/application/uploads:ro
```

Source code recoverable from Git, reproducible dependencies, and caches generally do not need to be backed up. Focus on data that cannot be recreated. If deployment secrets must be backed up, mount only the required files as read-only and strictly protect every backup destination.

Pack runs with UID/GID `10001`. Bind mounts used for `/backups` or `/home/pack/.pack` must be writable by this user.

## Adapt database backups

The `database_data` volume contains PostgreSQL's internal files, and `mysql_data` contains MySQL's internal files. Do not mount these volumes into Pack or archive them while the databases are running: copying live internal files can produce an inconsistent backup that is difficult to restore.

This example uses `pg_dump` and `mysqldump` over the Docker network to create consistent logical dumps. The database hosts in `pack.yml` must match the database service names in your own Compose file.

## Persistence and graceful shutdown

Each volume has a separate responsibility:

| Volume | Pack mount | Content |
| --- | --- | --- |
| `application_data` | `/source/application:ro` | Data produced by the application |
| `pack_state` | `/home/pack/.pack` | Logs and retention cycler state |
| `pack_backups` | `/backups` | Local storage artifacts |
| `database_data` | none | PostgreSQL internal files |
| `mysql_data` | none | MySQL internal files |

`stop_grace_period: 10m` gives Pack time to finish an active backup after receiving SIGTERM. Adjust this duration to the maximum expected backup time.

## Keep an off-site copy

The `pack_backups` volume remains on the same Docker host. It does not protect against complete server loss, theft, or destruction. Keep at least one copy off-site, for example through SFTP or SCP.

An optional SFTP destination is included as a commented block in `pack.yml`. Configure its variables in `.env`, then uncomment it:

```yaml
storages:
  local:
    type: local
    path: /backups
    keep: 7
  offsite:
    type: sftp
    host: $SFTP_HOST
    port: $SFTP_PORT
    path: $SFTP_PATH
    username: $SFTP_USERNAME
    password: $SFTP_PASSWORD
    keep: 30
```

The Compose example forwards `SFTP_HOST`, `SFTP_PORT`, `SFTP_PATH`, `SFTP_USERNAME`, and `SFTP_PASSWORD` from `.env` to the `pack` service. SCP can be configured in the same way. Prefer a read-only mounted SSH key when available, and regularly verify that backups can be restored.

## Automated test

From the repository root, the same example is used by the Docker pipeline test:

```bash
scripts/docker-compose-example-test.sh
```

The test initializes PostgreSQL, MySQL, and the application files, runs `pack perform`, and verifies that the artifact contains every expected marker.
