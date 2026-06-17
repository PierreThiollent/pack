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

- [ ] Étudier les tags de logs colorés
      Aujourd'hui les tags comme `[Config]`, `[Run]`, `[Archive]` ou `[Local]` sont du texte
      directement inclus dans le message de log. Pour les colorer proprement, il faudra choisir
      entre une approche simple avec des couleurs ANSI dans les messages, une macro/helper de log,
      ou une approche plus structurée avec un champ `tag` et un formatter dédié. À garder comme
      polish CLI, pas prioritaire pour le MVP.

- [x] Améliorer le format des dates dans les logs
      Aujourd'hui les logs affichent une date UTC avec suffixe `Z`, par exemple
      `2026-06-15T12:06:52.584023Z`, alors que l'heure locale peut être `14:06`.
      Pour un CLI utilisé manuellement, ce décalage est confus, surtout que les noms d'archives
      `tar.gz` utilisent déjà l'heure locale.
      Fait : les logs affichent maintenant l'heure locale avec offset explicite, sans fractions
      de seconde, par exemple `2026-06-15 14:06:52 +02:00`. L'UTC reste utile pour des logs
      machine, donc on décidera plus tard si on veut rendre ce format configurable.

- [x] Améliorer les messages affichés par le CLI
      Utiliser des formulations simples et cohérentes :
      `Starting backup run`, `Running model: ...`, `Dumping database: ...`,
      `Creating archive: ...`, `Uploading to storage: ...`, `Backup run completed`, etc.
      Fait : messages taggés inspirés de GoBackup (`[Config]`, `[Run]`, `[Model: ...]`,
      `[MySQL: ...]`, `[Archive]`, `[Storage: ...]`, `[Local]`, `[Cleanup]`).

- [ ] Remplacer plus tard `Result<T, String>` par des erreurs typées avec `thiserror`
      Ce n'est pas nécessaire pour démarrer le logging, mais ce sera utile pour afficher
      des erreurs plus structurées quand compression, FTP/SFTP et réseau seront ajoutés.

## Compressor

- [ ] V2 compressor : utiliser `pigz` automatiquement si disponible, sinon fallback `flate2`
      Pour la première version du compressor, on garde `flate2` : c'est simple, portable,
      testable, et suffisant pour valider le pipeline `dump → archive → tgz → storage`.
      Limite connue : la compression gzip actuelle est mono-threadée et peut devenir un
      goulot d'étranglement sur de gros dumps ou de grosses archives.

      Décision V2 : reproduire l'approche de GoBackup pour `tgz` / `tar.gz` : détecter `pigz`
      dans le `PATH`, l'utiliser quand il est disponible, et retomber automatiquement sur
      `flate2` sinon. Cela donne un gain multi-thread rapide sans compliquer la compilation
      ni la distribution de `pack` avec des dépendances natives intégrées.

      Points d'implémentation à prévoir :
      - détecter `pigz` proprement et logger le backend choisi ;
      - streamer `tar::Builder` vers `pigz.stdin` ;
      - écrire `pigz.stdout` vers le fichier `.tar.gz` ;
      - collecter/propager `stderr` et le code de sortie ;
      - garantir le fallback `flate2` si `pigz` est absent ;
      - ajouter des tests avec un faux binaire `pigz` dans un `PATH` temporaire.

      Piste future après V2 : évaluer `gzp`, la librairie utilisée par `crabz`, si on veut un
      backend gzip multi-threadé intégré au binaire plutôt qu'un process externe. `gzp` fournit
      un `Write` parallèle compatible avec notre pipeline streaming, mais ses versions récentes
      semblent privilégier des backends natifs C (`zlib-ng` / `libdeflate`) plutôt qu'un backend
      100% Rust pur. Il faudra donc benchmarker et vérifier l'impact sur la portabilité, la
      compilation, les releases et la surface de sécurité avant d'en faire un backend officiel.

## Archive

- [x] Vérifier que lorsque l'on archive un dossier, si une écriture/suppression a lieu pendant l'archive, cela ne fait pas planter le processus.
      Par exemple, si un fichier est supprimé pendant l'archive, `tar` peut échouer avec une erreur "file not found".
      Fait : pendant l'archive, pack ignore uniquement les erreurs `NotFound` sur les fichiers/dossiers qui disparaissent,
      loggue un warning, puis continue. Les autres erreurs restent bloquantes.
      L'include racine manquant au démarrage reste une erreur bloquante.

## Temp directory

- [x] Utiliser un dossier unique par run
      Au lieu de `/tmp/pack/{model}/` fixe, créer `{workdir_or_system_temp_dir}/pack-{timestamp}-{random}/{model}/`
      Évite les collisions si deux `perform` sont lancés en même temps

- [x] Ajouter un `workdir` configurable
      Comme GoBackup, permettre de choisir la racine des fichiers temporaires :
      `workdir: /var/tmp/pack`.
      Le dossier unique par run serait ensuite créé à l’intérieur de ce `workdir`.

- [x] Passer les chemins en `Path` / `PathBuf` plus loin dans le pipeline
      Aujourd'hui `database::run()` attend un `&str`, donc `model.rs` convertit le chemin avec
      `to_string_lossy()`. Plus tard, accepter directement un `&Path` serait plus idiomatique.

- [x] Sécuriser les noms utilisés dans les chemins temporaires
      Empêcher un nom de model bizarre comme `../foo` de sortir du dossier de run.
      Fait : les noms de models sont maintenant limités à `[A-Za-z0-9_-]+` avant de créer
      leur dossier temporaire.

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

## SFTP

- [x] Investiguer et améliorer les performances d'upload SFTP
      Test manuel OVH du 2026-06-17 : artifact `.tar.gz` d'environ 89 MiB.
      Avant optimisation : SFTP upload environ 2 min 45 s, alors que FTP et un client FTP/SFTP externe
      étaient beaucoup plus rapides sur le même serveur.

      Cause probable : `std::io::copy(...)` utilisait un buffer trop petit pour `ssh2::File` / libssh2,
      provoquant beaucoup de petits writes SFTP coûteux.

      Fait : remplacement par une copie manuelle avec buffer SFTP de 4 MiB, `write_all(...)`, `flush()`
      explicite et log de durée/débit. `TcpStream::set_nodelay(true)` est aussi activé.

      Mesures manuelles :
      - buffer 256 KiB : 89.03 MiB en 7.94 s, environ 11.22 MiB/s ;
      - buffer 1 MiB : 89.03 MiB en 2.74 s, environ 32.46 MiB/s ;
      - buffer 4 MiB : 89.03 MiB en 1.82 s, environ 48.97 MiB/s ;
      - buffer 8 MiB : 89.03 MiB en 1.70 s, environ 52.22 MiB/s.

      Décision : garder 4 MiB par défaut. Le gain de 8 MiB existe mais reste faible par rapport au coût
      mémoire et au côté plus agressif du tuning.

- [ ] Ajouter plus tard un upload atomique FTP/SFTP avec fichier temporaire `.part`
      À réserver pour une v0.2, v0.3 ou plus tard selon les priorités.
      Principe : uploader d'abord vers `backup.tar.gz.part`, puis renommer vers `backup.tar.gz`
      uniquement si l'upload a réussi. Avantage : éviter de laisser un fichier incomplet sous le nom
      final si l'upload est interrompu.

      À appliquer de façon cohérente au moins à FTP et SFTP, pas seulement à SFTP. Points à prévoir :
      rename distant, cleanup du `.part` en cas d'erreur, et comportement si le fichier final existe déjà.

- [ ] Vérifier la taille distante après upload SFTP
      Après l'upload, faire un `stat(...)` distant et comparer la taille distante avec la taille locale.
      Avantage : détecter un upload incomplet ou corrompu. Inconvénient : requête réseau supplémentaire.

- [ ] Ajouter une progression d'upload SFTP
      Ajouter plus tard des logs de progression ou une progress bar pour les gros artifacts.
      Le log final de débit existe déjà, mais il n'indique rien pendant un upload long.

- [ ] Implémenter l'authentification SFTP par clé privée
      Le parsing existe déjà (`private_key`, `passphrase`), mais l'auth réelle n'est pas encore branchée.
      À faire : expansion de `~`, auth sans passphrase, auth avec passphrase, erreurs claires si le fichier
      de clé est introuvable ou refusé.

- [ ] Documenter les chemins SFTP relatifs vs absolus
      Sur certains hébergements mutualisés, `path: /pack/backups` peut être interprété comme un chemin
      absolu système en SFTP, alors que FTP expose souvent une racine virtuelle. Documenter les exemples
      recommandés : `path: pack/backups` ou `path: /home/user/pack/backups` selon le serveur.

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

- [x] Renommer le projet et le binaire en `pack`
