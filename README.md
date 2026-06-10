<p align="center">
  <img src="https://img.icons8.com/emoji/96/backpack-emoji.png" width="96" alt="rbak" />
</p>

<h1 align="center">rbak</h1>

<p align="center">
  Back up databases and files to local or cloud storage from a single CLI.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/version-0.1.0-blue" alt="Version" />
  <img src="https://img.shields.io/badge/rust-2024-orange" alt="Rust 2024" />
  <img src="https://img.shields.io/badge/license-MIT-green" alt="License" />
</p>

rbak is a backup tool designed for application servers. It exports databases, packs files,
compresses the result, and stores the final archive outside the server.

The goal is simple: configure it once, run it from the command line or on a schedule, and keep
reliable backups without adding a heavy runtime dependency to your system.

The project is still early and is being built step by step in Rust.

## Features

Current features:

- YAML-based configuration.
- Multiple backup models in one config file.
- On-demand backups with `rbak perform`.
- MySQL dumps through the native `mysqldump` client.
- Temporary work directory per model with automatic cleanup.
- Clear error messages for missing config files, invalid YAML, and failed dumps.
- Unit and integration tests for the CLI and configuration parser.

Planned features:

- Back up files and directories into tar archives.
- Compress backups with `tgz`, `tbz2`, or `txz`.
- Upload backups to local and remote storage backends.
- Split large backup files into smaller parts.
- Encrypt backup archives with `age`.
- Rotate old backups with a retention policy.
- Run as a daemon with built-in scheduling.
- Send success and failure notifications.
- Expose a small web UI and REST API.

### Databases

- MySQL
- PostgreSQL
- Redis
- MongoDB
- SQLite
- MariaDB
- Microsoft SQL Server
- InfluxDB
- etcd
- Firebird

Only MySQL is implemented for now.

### Storages

- Local filesystem
- FTP
- SFTP
- SCP
- Amazon S3 / MinIO
- Google Cloud Storage
- Azure Blob Storage
- Backblaze B2
- WebDAV

Storage uploads are planned for the next milestones.

### Notifications

Planned notification backends include:

- Webhook
- Slack
- Discord
- Mail / SMTP
- Telegram

## Installation

For now, rbak is built from source:

```bash
git clone https://github.com/your-account/rbak
cd rbak
cargo build --release
cp target/release/rbak /usr/local/bin/
```

### Requirements

- [Rust](https://rustup.rs/) with the 2024 edition.
- Database client tools installed on the machine, for example `mysqldump` for MySQL.

## Configuration

rbak looks for its configuration file in:

- the path passed with `-c` / `--config`;
- `~/.rbak/rbak.yml` by default.

Minimal example:

```yml
models:
  my_site:
    databases:
      mysql:
        type: mysql
        host: localhost
        port: 3306
        database: myapp_prod
        username: root
        password: s3cr3t
```

A model is a backup unit. It usually maps to one application and can contain multiple databases.

```yml
models:
  app:
    databases:
      primary:
        type: mysql
        host: localhost
        port: 3306
        database: app_production
        username: root
        password: password

      analytics:
        type: mysql
        host: 10.0.0.5
        database: analytics
        username: backup
```

The configuration format will later include archives, compression, storages, scripts, schedules,
retention, encryption, and notifications.

## Usage

Run a backup with the default config path:

```bash
rbak perform
```

Run a backup with an explicit config file:

```bash
rbak perform -c /path/to/rbak.yml
```

Show help or version:

```bash
rbak --help
rbak --version
```

Example output:

```text
$ rbak perform -c rbak.yml
Model: my_site
  Database: mysql
  mysql done
```

On error:

```text
$ rbak perform -c rbak.yml
Model: my_site
  Database: mysql
Error: mysqldump failed:
mysqldump: Got error: 2002: Can't connect to local MySQL server...
```

The temporary directory is cleaned up even when the dump fails.

## Schedule

Scheduled backups are not implemented yet. The planned daemon mode will support both cron-like
schedules and simple interval-based schedules.

Example target configuration:

```yml
models:
  app:
    schedule:
      cron: "5 4 * * sun"
    databases:
      mysql:
        type: mysql
        host: localhost
        database: app_production
        username: root
        password: password
```

## Target pipeline

Each model is intended to run through this pipeline:

```text
Dump databases
  -> collect files
  -> create archive
  -> compress
  -> encrypt
  -> split
  -> upload
  -> rotate old backups
  -> clean up
```

Today, only the MySQL dump and cleanup steps are implemented.

## Roadmap

| Version | Goal |
|---|---|
| v0.1 | CLI, YAML configuration, MySQL dumps, model pipeline |
| v0.2 | Archive, compression, Local / FTP / SFTP storage |
| v0.3 | Encryption, split, retention |
| v0.4 | Daemon, scheduling, signal handling |
| v0.5 | Notifications |
| v0.6 | Web UI and REST API |
| v1.0 | Remaining databases, more storage backends, complete documentation |

See [PLAN.md](PLAN.md) for the full plan.

## Development

```bash
cargo test
cargo build
cargo fmt
```

The project's Git hooks check Rust formatting before commits.

## License

MIT
