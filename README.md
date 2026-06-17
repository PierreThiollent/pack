<p align="center">
  <img src="https://img.icons8.com/emoji/96/backpack-emoji.png" width="96" alt="pack" />
</p>

<h1 align="center">pack</h1>

<p align="center">
  Simple application backups from a single CLI.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version" />
  <img src="https://img.shields.io/badge/rust-2024-orange" alt="Rust 2024" />
  <img src="https://img.shields.io/badge/license-MIT-green" alt="License" />
</p>

`pack` exports databases, collects files, compresses the result, and stores the final archive in one or more destinations.

It is designed to be small, explicit, and easy to run from cron or any existing scheduler.

## Installation

Build from source:

```bash
git clone https://github.com/your-account/pack
cd pack
cargo build --release
cp target/release/pack /usr/local/bin/
```

Requirements:

- Rust 2024 edition.
- Database client tools installed on the machine, for example `mysqldump` for MySQL backups.

## Quick start

Install `pack` with the install script:

```bash
curl -fsSL https://raw.githubusercontent.com/your-account/pack/main/install.sh | sh
```

Then create a config file:

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

    archive:
      includes:
        - /var/www/my_site/uploads
        - /var/www/my_site/.env

    compress_with:
      type: tgz

    storages:
      local:
        type: local
        path: ~/backups/pack
```

Run the backup:

```bash
pack perform
```

Or use an explicit config path:

```bash
pack perform -c /path/to/pack.yml
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

### Archive includes

```yml
archive:
  includes:
    - /var/www/my_site/uploads
    - /var/www/my_site/.env
```

Included files and directories are added to an intermediate tar archive before compression.

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

### Local

```yml
storages:
  local:
    type: local
    path: ~/backups/pack
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
```

Required fields: `host`, `username`, `password`.

Defaults:

- `port`: `21`
- `timeout`: `300` seconds
- `path`: `/` — often the FTP user's virtual root directory, not the server filesystem root
- `explicit_tls`: `false`
- `no_check_certificate`: `false`

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
```

Required fields: `host`, `username`, and at least one authentication method: `password` or `private_key`.

Defaults:

- `port`: `22`
- `timeout`: `300` seconds
- `path`: `/` — the server root from the SFTP session point of view; on shared hosting this may not be writable

`passphrase` is only valid with `private_key` authentication.

For SFTP, `path: backups/my_site` is relative to the login directory, while `path: /backups/my_site` is an absolute server path. On shared hosting, relative paths are often the safer choice.

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

The project uses Git hooks to run formatting checks before commits.

## License

MIT
