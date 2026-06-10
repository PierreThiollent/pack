# rbak — Backup tool in Rust

**Objectif** : Construire un outil de sauvegarde en Rust pour apprendre le langage tout en produisant un binaire fonctionnel.

## Architecture cible

```
rbak/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entrypoint CLI
│   ├── config.rs            # Parsing YAML (serde + yaml-rust)
│   ├── model.rs             # Pipeline : dump → archive → compress → encrypt → split → upload
│   ├── database/            # Sources de données (trait Database)
│   │   ├── mod.rs           # Trait commun
│   │   ├── mysql.rs         # mysqldump
│   │   ├── postgresql.rs    # pg_dump
│   │   ├── redis.rs         # COPY / SAVE
│   │   ├── sqlite.rs        # .backup
│   │   ├── mongodb.rs       # mongodump
│   │   └── ...
│   ├── storage/             # Destinations (trait Storage)
│   │   ├── mod.rs           # Trait commun
│   │   ├── local.rs         # Copie locale
│   │   ├── s3.rs            # aws-sdk-s3
│   │   ├── ftp.rs           # FTP upload
│   │   ├── scp.rs           # SSH/SCP
│   │   ├── webdav.rs        # WebDAV
│   │   └── ...
│   ├── compressor.rs        # Compression (tar + gz/bz2/xz via flate2, bzip2, xz crates)
│   ├── archive.rs           # Tar
│   ├── encryptor.rs         # Chiffrement (age ou openssl)
│   ├── splitter.rs          # Découpage en chunks
│   ├── schedule.rs          # Planification (cron crate)
│   ├── notifier/            # Notifications (trait Notifier)
│   │   ├── mod.rs
│   │   ├── webhook.rs
│   │   ├── slack.rs
│   │   ├── discord.rs
│   │   ├── mail.rs
│   │   └── ...
│   ├── web.rs               # Serveur HTTP + API (axum ou actix-web)
│   └── cycler.rs            # Gestion de la rétention (keep)
```

**Dépendances Rust pressenties :**

- `clap` — CLI argument parsing
- `serde` + `serde_yaml` — Config YAML
- `tokio` — Async runtime
- `reqwest` — HTTP client (notifications, webhooks)
- `aws-sdk-s3` — S3 storage
- `ssh2` / `scp` — SFTP/SCP
- `flate2`, `bzip2`, `xz` — Compression
- `tar` — Archive
- `cron` — Schedule
- `axum` — Web UI
- `age` — Chiffrement (✅ **choisi** — natif Rust, moderne)
- `tracing` — Logging structuré
- `uuid` + `chrono` — Identifiants et dates
- `suppaftp` — FTP

---

## MVP (v0.1.0) — ~2-3 semaines

Fonctionnalités minimales pour avoir un outil utilisable en ligne de commande, capable de sauvegarder une base de données locale vers un stockage local ou FTP avec compression.

### CLI
- [ ] Binaire `rbak` avec sous-commandes : `perform`, `help`
- [ ] Chargement de la config depuis `~/.rbak/rbak.yml` ou chemin explicite `-c`
- [ ] Parsing YAML de la config (models → databases → storages → compress_with)

### Modèle / Pipeline
- [ ] Exécution d'un pipeline complet pour un model :
  - Dump database → Archive (tar) → Compress (gz) → Upload → Cleanup temp
- [ ] Cycle de vie : workdir temporaire → suppression après upload
- [ ] Logging console avec niveau (info, warn, error)

### Database
- [ ] **MySQL** : `mysqldump` en subprocess → dump.sql (✅ **choisi**)

### Archive
- [ ] Inclure les dossiers/fichiers listés dans `archive.includes` → tar
- [ ] Exclure ceux dans `archive.excludes`
- [ ] Combinaison : dump SQL + archive → tar final

### Compression
- [ ] `compress_with.type = "tgz"` → tar.gz
- [ ] `tbz2` et `txz` (bonus)

### Storage
- [ ] **Local** : copie du fichier vers `path` avec horodatage (✅ **choisi**)
- [ ] **FTP** : upload vers serveur FTP (via `suppaftp`) (✅ **choisi**)
- [ ] **SFTP** : upload via SSH (crate `ssh2`) (✅ **choisi**)

### Schedule
- [ ] `before_script` et `after_script` exécutés en shell
- [ ] Pas encore de daemon : `rbak perform` exécute une seule fois

### Tests
- [ ] Tests unitaires pour le parsing de config
- [ ] Tests d'intégration pour le pipeline (mock des subprocess)

### Livrable
- [ ] README.md avec exemple de config et usage
- [ ] Binaire unique fonctionnel sur macOS/Linux

---

## v0.2.0 — Storages avancés

- [ ] **SCP** : upload via SSH (crate `ssh2`)
- [ ] **WebDAV** : upload via HTTP WebDAV
- [ ] **S3** : upload vers AWS S3/MinIO (crate `aws-sdk-s3`)
- [ ] **GCS / Azure / B2** (un ou deux selon motivation)
- [ ] Multi-storage : upload vers plusieurs destinations en parallèle
- [ ] `default_storage` pour le Web UI

---

## v0.3.0 — Compression + Chiffrement + Split

- [ ] Compression : `tbz2` (bzip2) et `txz` (xz/lzma)
- [ ] **Encrypt** : chiffrement du fichier avant upload (crate `age` — natif Rust, moderne)
- [ ] **Splitter** : découpage en chunks de `chunk_size` avec extension `-NNN`
- [ ] **Cycler** : `keep: N` → lecture/écriture de `~/.rbak/cycler.json`, purge des backups les plus anciens

---

## v0.4.0 — Daemon + Schedule + Signals

- [ ] Sous-commande `rbak start` → daemon en arrière-plan
- [ ] Sous-commande `rbak run` → premier plan
- [ ] **Scheduler intégré** : cron (`5 4 * * sun`) OU intervalle (`every: 1day`, `at: 04:05`)
- [ ] **Signal handling** : SIGHUP → reload config, SIGQUIT/SIGTERM → graceful shutdown
- [ ] PID file pour tracking du daemon

---

## v0.5.0 — Notifications

Implémenter les notifiers un par un :

1. **Webhook** (POST JSON générique)
2. **Slack** (webhook Slack)
3. **Discord** (webhook Discord)
4. **Mail (SMTP)** (lettre email avec lettre)
5. **Telegram** (bot API)
6. Les autres (Feishu, DingTalk, GitHub Issue, etc.) à la demande

Chaque notifier implémente un trait `Notifier` avec `notify_success()` et `notify_failure()`.

---

## v0.6.0 — Web UI

- [ ] Serveur HTTP avec **axum**
- [ ] API REST :
  - `GET /api/models` — lister les models
  - `GET /api/backups/:model` — lister les backups d'un model
  - `POST /api/backups/:model/perform` — lancer un backup
- [ ] Interface web statique (HTML + JS vanilla ou template Tera)
- [ ] Authentification Basic Auth (config `web.username` / `web.password`)

---

## v1.0.0 — Toutes les databases + Finalisation

- [ ] **Redis** : `COPY` ou sauvegarde RDB
- [ ] **MongoDB** : `mongodump`
- [ ] **SQLite** : `.backup` ou copie du fichier
- [ ] **MariaDB** : identique à MySQL
- [ ] **MSSQL** : `sqlcmd`
- [ ] **InfluxDB** : export via API HTTP
- [ ] **etcd** : `etcdctl snapshot`
- [ ] **Firebird** : `gbak`
- [ ] Gestion des ENV vars dans la config (`$VAR` / `${VAR}`)
- [ ] Hot-reload config (SIGHUP)
- [ ] Documentation complète
- [ ] CI/CD (GitHub Actions), release binaire cross-plateforme

---

## Roadmap visuelle

```
MVP (v0.1.0)    →  CLI + MySQL + Local/FTP/SFTP + TGZ compression
    ↓
v0.2.0         →  S3 + SCP + WebDAV + Multi-storage
    ↓
v0.3.0         →  TBZ2/TXZ + Encrypt (age) + Split + Cycler
    ↓
v0.4.0         →  Daemon + Scheduler + Signal handling
    ↓
v0.5.0         →  Notifications (Webhook, Slack, Discord, Mail...)
    ↓
v0.6.0         →  Web UI + REST API
    ↓
v1.0.0         →  Toutes les databases restantes + polish
```


