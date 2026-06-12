# TODO — Améliorations repérées en cours de route

## Logging

- [x] Ajouter `tracing` et `tracing-subscriber`
      Remplacer progressivement les `println!` / `eprintln!` par des appels `tracing::info!`,
      `tracing::warn!`, `tracing::error!` et `tracing::debug!`.
      Objectif : avoir des messages homogènes avec des niveaux de log clairs.
      Fait : dépendances ajoutées, module `src/logging.rs` créé, initialisation dans `main.rs`.
      Décision : conserver le timestamp + niveau (`INFO`, `WARN`, `ERROR`) comme GoBackup.
      Décision : écrire les logs sur `stderr`, pour garder `stdout` disponible pour une future sortie machine/script.

- [x] Remplacer les premiers `println!` / `eprintln!` par `tracing`
      `src/model.rs` utilise maintenant `tracing::info!` pour le run directory, les models,
      les databases et les storages, et `tracing::warn!` pour les erreurs de cleanup.
      Il reste encore des anciens affichages dans `main.rs`, `config.rs`, `archive.rs` et `storage/local.rs`.

- [ ] Ajouter un flag `-v` / `--verbose`
      Par défaut, afficher les logs `info`, `warn` et `error`.
      Avec `--verbose`, activer aussi les logs `debug` pour faciliter le diagnostic.

- [ ] Améliorer les messages affichés par le CLI
      Utiliser des formulations simples et cohérentes :
      `Starting backup run`, `Running model: ...`, `Dumping database: ...`,
      `Creating archive: ...`, `Uploading to storage: ...`, `Backup run completed`, etc.

- [ ] Remplacer plus tard `Result<T, String>` par des erreurs typées avec `thiserror`
      Ce n'est pas nécessaire pour démarrer le logging, mais ce sera utile pour afficher
      des erreurs plus structurées quand compression, FTP/SFTP et réseau seront ajoutés.

## Archive

- [ ] Vérifier que lorsque l'on archive un dossier, si une ecriture/suppression a lieu pendant l'archive, cela ne fait pas planter le processus.
      Par exemple, si un fichier est supprimé pendant l'archive, `tar` peut échouer avec une erreur "file not found". Il faudrait vérifier que cela n'empêche pas l'archive de se terminer correctement, ou ajouter une option pour ignorer ces erreurs.
      J'ai deja rencontré ce probleme avec gobackup et c'etait chiant, donc je veux m'assurer que rbak gère ça proprement.

## Temp directory

- [x] Utiliser un dossier unique par run
      Au lieu de `/tmp/rbak/{model}/` fixe, créer `{workdir_or_system_temp_dir}/rbak-{timestamp}-{random}/{model}/`
      Évite les collisions si deux `perform` sont lancés en même temps

- [x] Ajouter un `workdir` configurable
      Comme GoBackup, permettre de choisir la racine des fichiers temporaires :
      `workdir: /var/tmp/rbak`.
      Le dossier unique par run serait ensuite créé à l’intérieur de ce `workdir`.

- [x] Passer les chemins en `Path` / `PathBuf` plus loin dans le pipeline
      Aujourd'hui `database::run()` attend un `&str`, donc `model.rs` convertit le chemin avec
      `to_string_lossy()`. Plus tard, accepter directement un `&Path` serait plus idiomatique.

- [ ] Sécuriser les noms utilisés dans les chemins temporaires
      Empêcher un nom de model bizarre comme `../foo` de sortir du dossier de run.

- [ ] Rendre le timestamp du dossier de run plus lisible
      Aujourd'hui le dossier utilise un timestamp Unix en secondes. Plus tard, on pourrait utiliser
      un format humain comme `2026-06-11-14-08-00`, sans complexifier maintenant.

## MySQL

- [x] Ajouter les defaults MySQL inspirés de GoBackup
      - `port: 3306`
      - `username: root`

- [ ] Ajouter plus tard les autres options MySQL supportées par GoBackup
      - `socket`
      - `tables`
      - `exclude_tables`
      - `all_databases`
      - `args` pour passer des options supplémentaires à `mysqldump`
        Exemple : `--single-transaction --quick`

## Config

- [ ] Supporter les variables d’environnement dans le YAML
      Comme GoBackup, permettre `$MYSQL_PASSWORD` ou `${MYSQL_PASSWORD}` dans la config,
      pour éviter de stocker les secrets en clair.

## Gestion d’erreurs

- [ ] Remplacer progressivement `Result<T, String>` par un type d’erreur projet
      Aujourd’hui les erreurs sont de simples chaînes. C’est simple pour apprendre, mais à terme
      on veut des erreurs typées, plus faciles à maintenir et à enrichir.

- [ ] Ajouter un module `src/error.rs`
      S’inspirer de Spacebot :
      `pub type Result<T> = std::result::Result<T, Error>;`
      puis utiliser `crate::error::Result<T>` dans les modules.

- [ ] Ajouter `thiserror`
      Définir une enum principale, par exemple :
      - `Config`
      - `Database`
      - `Archive`
      - `Storage`
      - `Io(#[from] std::io::Error)`
      - `Process`

- [ ] Évaluer si `anyhow` est utile
      `thiserror` est adapté pour les erreurs structurées du projet.
      `anyhow` peut être utile pour ajouter rapidement du contexte dans un binaire CLI,
      mais on peut commencer avec `thiserror` seul.

- [ ] Faire cette refacto au bon moment
      Pas maintenant au milieu de l’archive. Bon moment possible :
      - après l’archive complète
      - avant compression
      - ou avant FTP/SFTP, quand les erreurs réseau/process deviendront plus nombreuses.

## Renommer le nom du CLI

- [x] Renommer le projet et le binaire en `rbak`
