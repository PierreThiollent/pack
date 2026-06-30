# pack — Backup tool in Rust

**Objectif** : Construire un outil de sauvegarde en Rust pour apprendre le langage tout en produisant un binaire fonctionnel.

## Architecture cible

```
pack/
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
│   ├── compressor.rs        # Compression (tar + gzip parallèle via gzp, puis bz2/xz plus tard)
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
- `gzp` — Compression gzip parallèle intégrée
- `bzip2`, `xz` — Compression bonus plus tard
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

- [x] Binaire `pack` avec sous-commandes : `perform`, `help`
- [x] Chargement de la config depuis `~/.pack/pack.yml` ou chemin explicite `-c`
- [x] Parsing YAML de la config (models → databases → storages → compress_with)

### Modèle / Pipeline

- [x] Exécution d'un pipeline complet pour un model :
  - Dump database → Archive (tar) → Compress (gz) → Upload → Cleanup temp
- [x] Cycle de vie : workdir temporaire unique par run → suppression après upload
- [x] Logging console avec niveau (info, warn, error)

### Database

- [x] **MySQL** : `mysqldump` en subprocess → dump SQL (✅ **choisi**)

### Archive

- [x] Inclure les dossiers/fichiers listés dans `archive.includes` → tar
- [x] Combinaison : dump SQL + archive → artifact final compressé

Note : `archive.excludes` est repoussé après le MVP pour garder v0.1.0 simple.

### Compression

- [x] `compress_with.type = "tgz"` → tar.gz
- [x] `tgz` utilise `gzp + deflate_rust` niveau 4 pour une compression gzip parallèle sans dépendance runtime externe
- [ ] `tbz2` et `txz` (bonus, repoussé après v0.1.0)

### Storage

- [x] **Local** : copie du fichier vers `path` avec horodatage (✅ **choisi**)
- [x] **FTP** : upload vers serveur FTP (via `suppaftp`) (✅ **choisi**)
- [x] **SFTP** : upload via SSH (crate `ssh2`) (✅ **choisi**)

### Schedule / scripts

- [ ] `before_script` et `after_script` exécutés en shell (repoussé en v0.2.0)
- [x] Pas encore de daemon : `pack perform` exécute une seule fois

### Tests

- [x] Tests unitaires pour le parsing de config
- [x] Tests d'intégration pour le pipeline local via le CLI
- [ ] Tests d'intégration avec mock des subprocess (optionnel / à réévaluer après v0.1.0)

### Livrable

- [x] README.md avec exemple de config et usage
- [x] Binaire unique fonctionnel sur macOS/Linux

---

## v0.2.0 — Archive exclude + Cycling + Daemon + Schedule + Signals +

- [x] Ajouter `archive.excludes` pour exclure certains fichiers/dossiers des archives
- [x] **Cycler** : `keep: N` par storage → état dans `~/.pack/cycler/{model}_{storage}.json`, purge des backups les plus anciens
- [x] Sous-commande `pack start` → daemon en arrière-plan
  - Daemonisation via `daemonize`.
  - Logs runtime dans `~/.pack/pack.log`.
  - PID file dans `~/.pack/pack.pid`.
  - Le daemon conserve le répertoire de lancement comme working directory, comme GoBackup.
- [x] Sous-commande `pack run` → scheduler au premier plan
  - Charge la config puis démarre un `tokio-cron-scheduler`.
  - Arrêt au premier plan via `Ctrl+C`.
- [ ] **Scheduler intégré**
  - [x] Support `schedule.cron` par model.
  - [x] Compatibilité cron 5 champs type GoBackup / crontab.guru (`5 4 * * sun`) via normalisation vers le format 6 champs attendu par `tokio-cron-scheduler` (`0 5 4 * * sun`).
  - [x] Exécution ciblée du model schedulé via `model::run_one`, sans relancer tous les models.
  - [x] Protection anti-concurrence : si un backup schedulé tourne déjà, les déclenchements suivants sont skippés.
  - [ ] Support intervalle (`every: 1day`, `at: 04:05`).
  - [ ] Améliorer l'UX de shutdown : log explicite pendant l'attente d'un backup en cours, deuxième `Ctrl+C` pour forcer l'arrêt.
  - [ ] Filtrer les logs internes trop verbeux de `tokio-cron-scheduler` (`Uninited`, `Job creator created`).
- [ ] **Signal handling** : SIGHUP → reload config, SIGQUIT/SIGTERM → graceful shutdown
- [x] PID file pour tracking du daemon

---

## v0.3.0 — Compression + Chiffrement + Split + before / after scripts

- [ ] Compression : `tbz2` (bzip2) et `txz` (xz/lzma)
- [ ] **Encrypt** : chiffrement du fichier avant upload (crate `age` — natif Rust, moderne)
- [ ] **Splitter** : découpage en chunks de `chunk_size` avec extension `-NNN`
- [ ] Upload atomique FTP/SFTP avec fichier `.part` puis rename
- [ ] Vérification optionnelle de taille distante après upload
- [ ] `before_script` et `after_script` exécutés en shell

---

## v0.4.0 — Storages avancés

- [ ] **SCP** : upload via SSH (crate `ssh2`)
- [ ] **WebDAV** : upload via HTTP WebDAV
- [ ] **S3** : upload vers AWS S3/MinIO (crate `aws-sdk-s3`)
- [ ] **GCS / Azure / B2** (un ou deux selon motivation)
- [ ] Multi-storage : upload vers plusieurs destinations en parallèle
- [ ] `default_storage` pour le Web UI

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
- [ ] Hot-reload config (SIGHUP)
- [ ] Documentation complète

---
