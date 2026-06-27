<p align="center">
  <img src="https://img.icons8.com/emoji/96/backpack-emoji.png" width="96" alt="pack" />
</p>

<h1 align="center">pack</h1>

<p align="center">
  Back up your apps. Pack them away.<br />
  Dump databases, archive files, compress, and store backups from one CLI.
</p>

<p align="center">
  <a href="https://github.com/PierreThiollent/pack/actions?query=workflow%3ACI">
    <img src="https://github.com/PierreThiollent/pack/actions/workflows/ci.yml/badge.svg" alt="Build & test" />
  </a>
  <a href="https://github.com/PierreThiollent/pack/releases">
    <img src="https://img.shields.io/github/v/release/PierreThiollent/pack" alt="Latest release" />
  </a>
  <img src="https://img.shields.io/badge/rust-2024-orange" alt="Rust 2024" />
  <img src="https://img.shields.io/badge/license-MIT-green" alt="License" />
</p>

`pack` is a small, explicit backup CLI for application servers.

It dumps databases, archives files, compresses everything into timestamped artifacts, and ships them to local, FTP, or SFTP storage.

## Installation

Install the latest release:

```bash
curl -fsSL https://raw.githubusercontent.com/PierreThiollent/pack/main/install.sh | sh
```

Install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/PierreThiollent/pack/main/install.sh | sh -s v0.1.0
```

The script downloads the matching release binary from GitHub Releases and installs it to `/usr/local/bin/pack`.

### Build from source

If you prefer to build from source:

```bash
git clone https://github.com/PierreThiollent/pack
cd pack
cargo build --release
cp target/release/pack /usr/local/bin/
```

Requirements:

- Rust 2024 edition.
- Database client tools installed on the machine, for example `mysqldump` for MySQL backups.

## Quick start

Create a config file:

```bash
mkdir -p ~/.pack
$EDITOR ~/.pack/pack.yml
```

Example:

```yml
workdir: /tmp

models:
  my_site:
    databases:
      mysql:
        type: mysql
        host: localhost
        port: 3306
        database: myapp_prod
        username: backup
        password: secret

    schedule:
      cron: "5 4 * * sun"

    archive:
      includes:
        - /var/www/my_site/uploads
        - /var/www/my_site/.env
      excludes:
        - /var/www/my_site/uploads/cache
        - /var/www/my_site/uploads/tmp

    compress_with:
      type: tgz

    storages:
      local:
        type: local
        path: ~/backups/pack
        keep: 7
```

Run the backup once:

```bash
pack perform
```

Run the scheduler in the foreground:

```bash
pack run
```

Or use an explicit config path:

```bash
pack perform -c /path/to/pack.yml
pack run -c /path/to/pack.yml
```

## Configuration

`pack` reads its configuration from:

- the path passed with `-c` / `--config`;
- `~/.pack/pack.yml` by default.

A `model` is one backup unit. It usually maps to one application.

Model names must only contain letters, digits, `_`, or `-`.

### Temporary work directory

`workdir` controls where temporary files are written. If it is not set, the system temporary directory is used.

Each run creates a unique directory:

```text
{workdir_or_system_temp_dir}/pack-{timestamp}-{random}/
```

The directory is removed at the end of the run, even when the backup fails.

### MySQL

```yml
databases:
  mysql:
    type: mysql
    host: localhost
    port: 3306
    database: myapp_prod
    username: backup
    password: secret
```

`database` is required. `host` defaults to `localhost`, `port` defaults to `3306`, and `username` defaults to `root`.

### Archive includes and excludes

```yml
archive:
  includes:
    - /var/www/my_site/uploads
    - /var/www/my_site/.env
  excludes:
    - /var/www/my_site/uploads/cache
    - /var/www/my_site/uploads/tmp
```

Files and directories listed in `includes` are added to an intermediate tar archive before compression.

`excludes` lets you skip files or directories inside the configured includes. Paths support `~` expansion. When an excluded path is a directory, all files and directories below it are skipped too.

An excluded path does not have to exist: if it matches nothing, it is simply ignored.

### Schedule

A model can define a `schedule` block to run automatically with `pack run`:

```yml
models:
  my_site:
    schedule:
      cron: "5 4 * * sun"
```

Cron expressions use the common 5-field syntax:

```text
minute hour day-of-month month day-of-week
```

For example, `5 4 * * sun` runs every Sunday at 04:05.

`pack` also accepts the 6-field format used internally by `tokio-cron-scheduler`:

```text
second minute hour day-of-month month day-of-week
```

If a scheduled backup is still running when the next cron tick happens, the new run is skipped to avoid concurrent backups.

`pack run` keeps running after a scheduled backup failure. The error is logged and the next cron tick will try again.

Interval schedules such as `every: 1day` / `at: 04:05` are planned but not implemented yet.

### Compression

```yml
compress_with:
  type: tgz
```

This produces a `.tar.gz` artifact named with the model and timestamp, for example:

```text
my_site-20260617-134625.tar.gz
```

## Storages

Storages define where final backup artifacts are copied or uploaded. A model can use one or more storages.

### Local

```yml
storages:
  local:
    type: local
    path: ~/backups/pack
    keep: 7
```

The final `.tar.gz` artifact is copied to the configured directory.

### FTP

```yml
storages:
  ftp:
    type: ftp
    host: ftp.example.com
    port: 21
    timeout: 300
    path: /backups/my_site
    username: pack
    password: secret
    explicit_tls: false
    no_check_certificate: false
    keep: 7
```

Required fields: `host`, `username`, `password`.

Defaults:

- `port`: `21`
- `timeout`: `300` seconds
- `path`: `/` — often the FTP user's virtual root directory, not the server filesystem root
- `explicit_tls`: `false`
- `no_check_certificate`: `false`
- `keep`: `0`

Notes:

- Remote directories are created automatically when needed.
- Transfers use binary mode.
- `explicit_tls: true` enables explicit FTPS when the server supports it.

### SFTP

Password authentication:

```yml
storages:
  sftp:
    type: sftp
    host: sftp.example.com
    port: 22
    timeout: 300
    path: backups/my_site
    username: pack
    password: secret
    keep: 7
```

Private key authentication:

```yml
storages:
  sftp:
    type: sftp
    host: sftp.example.com
    port: 22
    timeout: 300
    path: backups/my_site
    username: pack
    private_key: ~/.ssh/id_rsa
    passphrase: optional-passphrase
    keep: 7
```

Required fields: `host`, `username`, and at least one authentication method: `password` or `private_key`.

Defaults:

- `port`: `22`
- `timeout`: `300` seconds
- `path`: `/` — the server root from the SFTP session point of view; on shared hosting this may not be writable
- `keep`: `0`

`passphrase` is only valid with `private_key` authentication.

For SFTP, `path: backups/my_site` is relative to the login directory, while `path: /backups/my_site` is an absolute server path. On shared hosting, relative paths are often the safer choice.

## Retention / cycler

Each storage can define a `keep` value:

```yml
storages:
  local:
    type: local
    path: ~/backups/pack
    keep: 7
```

`keep: N` keeps the latest `N` backups known by that storage and removes older ones after a successful upload.

`keep: 0` means unlimited retention: backups are never removed automatically. This is the default when `keep` is omitted.

`pack` stores retention state locally in:

```text
~/.pack/cycler/{model}_{storage}.json
```

The cycler only removes backups that are present in this state file. Files that already exist in a storage but are not listed in the cycler state are left untouched.

If deleting an old backup fails, the backup run still succeeds. `pack` logs a warning and keeps that backup in the cycler state so it can retry deletion on a future run.

## Commands

### `pack perform`

Runs all configured models once, then exits.

If a backup fails, the command exits with an error code.

### `pack run`

Runs the scheduler in the foreground.

Only models with a `schedule` block are registered. When a scheduled job fails, the error is logged and the scheduler keeps running.

Stop it with `Ctrl+C`.

`pack start` for background daemon mode is planned for a future version.

## Logs

Example output:

```text
2026-06-17 13:46:25 +02:00  INFO [Run] Starting backup run
2026-06-17 13:46:25 +02:00  INFO [Model: my_site] Running model
2026-06-17 13:46:25 +02:00  INFO [MySQL: mysql] Dumping database
2026-06-17 13:46:26 +02:00  INFO [Archive] Archive created: /tmp/pack-.../my_site/archive.tar
2026-06-17 13:46:27 +02:00  INFO [Compressor] Compressed backup: /tmp/pack-.../my_site-20260617-134625.tar.gz
2026-06-17 13:46:28 +02:00  INFO [SFTP] Store succeeded: backups/my_site/my_site-20260617-134625.tar.gz
2026-06-17 13:46:28 +02:00  INFO [Run] Backup run completed
```

Set `RUST_LOG` to control log verbosity:

```bash
RUST_LOG=warn pack perform
```

## Development

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

## License

MIT
