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

- [x] Colorer les tags de logs proprement
      Les tags comme `[Config]`, `[Run]`, `[Archive]` ou `[Local]` sont maintenant rendus via
      `logging::tag(LogTag::...)` au lieu d'être du texte libre recopié dans chaque message.
      Décision : garder le formatter `tracing` standard et centraliser seulement le rendu des tags
      dans `src/logging.rs`, plutôt que d'ajouter un formatter custom trop lourd pour ce polish CLI.
      Les couleurs sont activées uniquement sur terminal, respectent `NO_COLOR`, et peuvent être
      forcées avec `FORCE_COLOR=1`.

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

- [ ] Réduire la redondance des logs storage
      Aujourd'hui l'orchestrateur loggue `[Storage: sftp] Uploading backup`, puis le backend concret
      loggue aussi `[SFTP] Uploading backup: remote/path.tar.gz`. C'est clair, mais un peu répétitif.
      Plus tard, décider si l'orchestrateur doit seulement annoncer le storage (`Running storage`) ou
      si le backend concret doit porter tous les logs d'upload détaillés.

- [ ] Filtrer les logs internes des dépendances du scheduler
      `tokio-cron-scheduler` émet actuellement des logs internes comme `Uninited` et
      `Job creator created` au niveau INFO. Ils polluent la sortie CLI de `pack run`.
      Plus tard, ajuster le filtre `tracing` pour conserver les logs INFO de pack tout en
      masquant les logs INFO trop verbeux des dépendances.

- [ ] Remplacer plus tard `Result<T, String>` par des erreurs typées avec `thiserror`
      Ce n'est pas nécessaire pour démarrer le logging, mais ce sera utile pour afficher
      des erreurs plus structurées quand compression, FTP/SFTP et réseau seront ajoutés.

## Compressor

- [ ] Approfondir le choix du timestamp dans les noms d'artifacts
      Aujourd'hui les artifacts utilisent un timestamp local implicite, par exemple
      `my_site-20260617-134625.tar.gz`. C'est lisible pour un humain sur la machine qui lance
      `pack`, mais le fuseau horaire n'est pas inscrit dans le nom.

      Problème rencontré en CI : les tests qui convertissaient un instant `+02:00` vers `Local`
      échouaient sur GitHub Actions, car les runners sont en UTC. Le fix actuel garde le format
      local implicite et rend les tests indépendants du timezone de la machine, mais le sujet mérite
      une vraie décision produit plus tard.

      Points à comparer :
      - heure locale implicite : `20260617-134625` ;
      - heure locale avec offset : `20260617-134625+0200` ;
      - UTC explicite : `20260617-114625Z` ;
      - format ISO-like sans caractères problématiques pour les fichiers ;
      - lisibilité humaine vs standard infra ;
      - tri lexical, machines dans plusieurs fuseaux, DST, restauration et audit.

      Décision v0.1 temporaire : conserver le format actuel local implicite pour ne pas changer
      l'expérience utilisateur juste avant la release.

- [x] Accélérer `tgz` avec un backend gzip parallèle intégré
      Piste initiale : utiliser `pigz` automatiquement si disponible, sinon fallback `flate2`.
      Décision finale : éviter une dépendance runtime externe et utiliser `gzp` directement dans
      le binaire.

      Benchmarks locaux sur un dataset SQL-like d'environ 710 MB :

| Backend              | Niveau |   Temps |        Débit | Taille artifact |                      Comparaison |
| -------------------- | -----: | ------: | -----------: | --------------: | -------------------------------: |
| `flate2` actuel      | défaut | ~4.49 s |   ~151 MiB/s |       ~31.5 MiB |                         baseline |
| `gzp + deflate_rust` |      3 | ~0.35 s | ~1 948 MiB/s |       ~39.8 MiB |  ~12.8x plus rapide, +26% taille |
| `gzp + deflate_rust` |      4 | ~0.42 s | ~1 628 MiB/s |       ~33.9 MiB | ~10.8x plus rapide, +7.6% taille |
| `gzp + deflate_rust` |      5 | ~0.47 s | ~1 440 MiB/s |       ~33.5 MiB |  ~9.6x plus rapide, +6.3% taille |
| `gzp + deflate_rust` |      6 | ~0.85 s |   ~792 MiB/s |       ~32.7 MiB |  ~5.3x plus rapide, +3.8% taille |

      Choix : `gzp + deflate_rust` niveau 4, très rapide et avec une taille d'archive encore
      raisonnable. `flate2` n'est plus une dépendance directe de `pack`; il reste seulement une
      dépendance transitive de `gzp` via `flate2/rust_backend`.

      Note : `zlib-ng` / `libdeflate` a été testé comme piste native, mais `zlib-ng` nécessite
      `cmake` au build sur la machine testée. On garde donc le backend Rust pur pour la
      portabilité.

## Daemon / logs

- [ ] Améliorer plus tard le message de démarrage concurrent de `pack start`
      Test manuel : un premier `pack start` démarre correctement en arrière-plan, crée
      `~/.pack/pack.pid` et écrit dans `~/.pack/pack.log`. Si un deuxième `pack start` est lancé
      pendant que le premier tourne, `daemonize` échoue bien à locker le PID file côté child
      (`unable to lock pid file`) et l'erreur est visible dans le terminal. Le comportement est donc
      compréhensible, mais l'UX reste un peu contradictoire car le parent affiche d'abord
      `Pack daemon started.`.

      Amélioration possible plus tard : ajouter un petit handshake parent/child avec un pipe Unix.
      Le parent créerait le pipe avant `daemonize`, puis attendrait un message du child avant
      d'afficher le succès. Le child écrirait par exemple `READY` dans le pipe seulement après les
      étapes minimales : PID file locké/écrit par `daemonize`, logging fichier initialisé, config
      chargée, scheduler démarré. Si le child échoue avant `READY`, le parent pourrait afficher une
      erreur claire au lieu de `Pack daemon started.`.

      Pour l'instant, ne pas implémenter ce handshake : le comportement actuel est acceptable et on
      garde le code simple.

- [x] Tester `pack start` avec la vraie config
      Vérifié avec `/Users/pierrethiollent/.pack/pack.yml` : la commande rend la main immédiatement,
      `~/.pack/pack.pid` contient un PID non vide, les logs runtime vont dans `~/.pack/pack.log`,
      et le job cron `*/3 * * * *` déclenche bien le backup en arrière-plan. Le backup a échoué au
      moment de l'upload SFTP parce que Tailscale n'était pas lancé, ce qui est normal pour ce test.

- [x] Vérifier que le PID file pointe vers le vrai daemon
      Après `pack start`, lancer `ps -p $(cat ~/.pack/pack.pid) -o pid,ppid,command` et vérifier que
      le process existe encore, que le PID correspond au daemon et pas au parent déjà terminé.

- [x] Vérifier que `pack start -c <path>` utilise le bon fichier de config
      Vérifié avec une config temporaire : `pack start -c <path>` écrit bien dans
      `~/.pack/pack.log` une ligne `Loaded config from custom path: ...` pointant vers le fichier
      attendu.

- [x] Vérifier que les logs runtime de `pack start` ne sortent pas dans la console
      Vérifié avec une config temporaire : stdout contient uniquement les 3 lignes du parent
      (`Pack daemon started`, log file, PID file), stderr est vide, et les logs runtime du
      child (`Loaded config`, `Started in background`, scheduler) vont uniquement dans
      `~/.pack/pack.log`.

- [x] Tester les chemins relatifs en mode daemon
      Vérifié avec une config temporaire lancée depuis son répertoire : `workdir: ./workdir`,
      `archive.includes: ./source/file.txt` et storage local `path: ./backups`. Comme GoBackup,
      `pack start` conserve le répertoire de lancement comme working directory du daemon. Le backup
      schedulé a réussi et l'artifact a été créé dans le dossier relatif attendu `./backups`.

- [x] Tester l'arrêt du daemon pendant un backup en cours
      Vérifié avec `/Users/pierrethiollent/.pack/pack.yml` pendant un backup réel. Après
      `kill $(cat ~/.pack/pack.pid)`, pack loggue bien `[Run] Received shutdown signal, stopping
      scheduler...`, laisse le backup en cours se terminer, upload l'artifact SFTP, loggue
      `Backup run completed`, puis le process s'arrête. Le dossier temporaire du run
      `/Users/pierrethiollent/Desktop/pack-1782803700-fT5Z3w` a bien été supprimé.

- [ ] Tester `pack start` avec un binaire release/installé
      Ne pas valider uniquement avec `cargo run -- start`. Tester aussi avec le binaire installé, car
      daemonize/fork peut révéler des différences selon le contexte d'exécution.

- [ ] Ajouter plus tard un test d'intégration automatisé pour `pack start`
      Quand le comportement concurrent et le cleanup du PID file seront stabilisés : créer une config
      temporaire, lancer `pack start -c ...`, attendre le PID file et une ligne de log, tuer le daemon,
      puis nettoyer le PID file. Ne pas ajouter ce test trop tôt pour éviter un test flaky.

- [x] Tester l'arrêt du daemon `pack start`
      Vérifié avec une config temporaire : `kill $(cat ~/.pack/pack.pid)` arrête bien le process
      daemon (`ps` ne retrouve plus le PID après l'arrêt). En revanche, aucun log d'arrêt propre
      n'est écrit pour l'instant et `~/.pack/pack.pid` reste présent avec l'ancien PID. Il faudra
      ajouter le signal handling/cleanup explicite, ou décider de s'appuyer uniquement sur le lock
      libéré par le système.

- [x] Ajouter un log d'arrêt et un shutdown propre pour `pack start`
      Fait : `scheduler::wait_for_shutdown_signal()` écoute maintenant `SIGTERM` en plus de
      `Ctrl+C` / `SIGINT`, via `tokio::signal::unix::signal(SignalKind::terminate())` et
      `tokio::select!`.

      Test manuel validé avec `pack start -c <config temporaire>` puis
      `kill $(cat ~/.pack/pack.pid)` : le process s'arrête et `~/.pack/pack.log` contient bien
      `[Run] Received shutdown signal, stopping scheduler...` avant l'arrêt. Le PID file peut rester
      sur disque : c'est acceptable car `daemonize` utilise un lock système libéré à la mort du
      process et réécrit le fichier au prochain démarrage.

- [ ] Ajouter une commande `pack stop`
      Plus tard, lire `~/.pack/pack.pid`, envoyer un signal d'arrêt au daemon, afficher un message
      clair, puis gérer les cas : aucun PID file, process déjà arrêté, PID file stale, permission
      refusée. Cette commande remplacera l'usage manuel de `kill $(cat ~/.pack/pack.pid)`.

- [x] Tester le comportement avec un PID file stale
      Vérifié en écrivant un faux ancien PID (`999999`) dans `~/.pack/pack.pid` avant de relancer
      `pack start -c <config temporaire>`. `daemonize` redémarre correctement, remplace le contenu
      du PID file par le nouveau PID du daemon, et le scheduler démarre. Le fichier peut donc rester
      sur disque après arrêt : le point important est que le lock système soit libéré quand le process
      meurt.

- [x] Tester `pack start` avec un cron fréquent
      Vérifié avec une config temporaire, un petit fichier archivé, un storage local temporaire et
      `cron: "*/10 * * * * *"`. Le daemon a déclenché deux backups successifs à 10 secondes
      d'intervalle, créé deux artifacts `.tar.gz`, et les logs sont restés lisibles dans
      `~/.pack/pack.log`.

- [x] Tester les erreurs de démarrage de `pack start`
      Testé avec un fichier de config absent et un YAML invalide. L'erreur est bien écrite dans
      `~/.pack/pack.log` côté child (`Failed to read config file` / `Failed to parse config file`),
      et le process daemon meurt ensuite. En revanche, le parent affiche `Pack daemon started.` et
      `~/.pack/pack.pid` reste avec le PID mort. Pour l'instant, on accepte ce fonctionnement simple
      inspiré de GoBackup : utiliser `pack run` pour valider la config avant de lancer le daemon.

- [ ] Ajouter une rotation des logs fichier
      `pack run` écrit maintenant aussi dans `~/.pack/pack.log`, et `pack start` utilisera ce
      fichier pour le mode daemon. Comme GoBackup, la première version est append-only, sans
      rotation. Plus tard, ajouter une rotation pour éviter une croissance illimitée : taille max,
      nombre de fichiers conservés, et éventuellement âge max.

## Release / binaire

- [ ] Optimiser la taille du binaire release
      Le binaire release est déjà autour de 6 MB, ce qui reste raisonnable vu les dépendances
      embarquées (`tokio`, scheduler, FTP/SFTP, TLS, YAML, tracing, `gzp`, etc.).
      Plus tard, tester un profil release optimisé taille :
      `strip = true`, `lto = true`, `codegen-units = 1`, éventuellement `panic = "abort"`.
      Mesurer le gain réel et l'impact sur le temps de compilation avant de l'activer.

## Archive

- [x] Vérifier que lorsque l'on archive un dossier, si une écriture/suppression a lieu pendant l'archive, cela ne fait pas planter le processus.
      Par exemple, si un fichier est supprimé pendant l'archive, `tar` peut échouer avec une erreur "file not found".
      Fait : pendant l'archive, pack ignore uniquement les erreurs `NotFound` sur les fichiers/dossiers qui disparaissent,
      loggue un warning, puis continue. Les autres erreurs restent bloquantes.
      L'include racine manquant au démarrage reste une erreur bloquante.

- [ ] Logger un warning quand un chemin `archive.excludes` ne matche rien
      Aujourd'hui un exclude inexistant est ignoré sans erreur, ce qui est volontaire : un dossier
      optionnel comme `cache`, `tmp` ou `.git` ne doit pas faire échouer le backup s'il n'existe pas.
      Amélioration possible : logger un warning si un chemin listé dans `archive.excludes` ne correspond
      à aucun fichier/dossier parcouru, afin de détecter les typos sans stopper le backup.

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

- [x] Ajouter les defaults MySQL inspirés de GoBackup - `port: 3306` - `username: root`

- [ ] Ajouter plus tard les autres options MySQL supportées par GoBackup - `socket` - `tables` - `exclude_tables` - `all_databases` - `args` pour passer des options supplémentaires à `mysqldump`
      Exemple : `--single-transaction --quick`

## Cycler / rétention

- [x] Clarifier la politique quand la suppression storage échoue
      Décision : pack garde dans le state les backups que le storage n'a pas réussi à supprimer.
      Le cycler calcule les candidats à supprimer, tente les suppressions, puis retire du state
      uniquement les clés supprimées avec succès. C'est plus strict que GoBackup, qui retire les
      entrées du state avant de supprimer physiquement les fichiers.

- [x] Documenter le fonctionnement du cycler dans le README
      Expliquer `keep`, `keep: 0`, l'emplacement du state `~/.pack/cycler/`, le fait que le cycler
      ne supprime que les fichiers connus dans son state, et le comportement warning-only quand une
      suppression échoue.

- [ ] Étudier une commande de réconciliation / prune des fichiers orphelins
      Le cycler ne supprime que les backups présents dans son state. Un fichier déjà présent dans le
      storage mais absent du JSON cycler reste donc en place, comme dans GoBackup. Plus tard, ajouter
      éventuellement une commande du type `pack cycler reconcile` ou `pack storage prune` pour lister
      ou nettoyer ces fichiers orphelins explicitement.

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

- [x] Implémenter l'authentification SFTP par clé privée
      Le parsing existait déjà (`private_key`, `passphrase`), mais l'auth réelle n'était pas encore branchée.
      Fait : expansion de `~`, auth avec `ssh2::Session::userauth_pubkey_file`, support de la passphrase,
      validation des clés vides, erreur claire si le fichier de clé est introuvable ou refusé.
      Test manuel validé avec une vraie clé privée sur serveur SFTP OVH/Tailscale.

- [ ] Documenter les chemins SFTP relatifs vs absolus
      Sur certains hébergements mutualisés, `path: /pack/backups` peut être interprété comme un chemin
      absolu système en SFTP, alors que FTP expose souvent une racine virtuelle. Documenter les exemples
      recommandés : `path: pack/backups` ou `path: /home/user/pack/backups` selon le serveur.

- [ ] Tester manuellement l'upload SFTP par clé privée + passphrase. Tester également avec une clé privée + passphrase incorrecte. Tester egalement avec clé privée inexistante et clé privée refusée par le serveur.

## Config

- [x] Supporter les variables d’environnement dans le YAML
      Comme GoBackup, permettre `$MYSQL_PASSWORD` ou `${MYSQL_PASSWORD}` dans la config,
      pour éviter de stocker les secrets en clair.
      Fait : expansion avant parsing YAML via `shellexpand`, avec erreur si une variable est absente.

## Gestion d’erreurs

- [ ] Remplacer progressivement `Result<T, String>` par un type d’erreur projet
      Aujourd’hui les erreurs sont de simples chaînes. C’est simple pour apprendre, mais à terme
      on veut des erreurs typées, plus faciles à maintenir et à enrichir.

- [ ] Ajouter un module `src/error.rs`
      S’inspirer de Spacebot :
      `pub type Result<T> = std::result::Result<T, Error>;`
      puis utiliser `crate::error::Result<T>` dans les modules.

- [ ] Ajouter `thiserror`
      Définir une enum principale, par exemple : - `Config` - `Database` - `Archive` - `Storage` - `Io(#[from] std::io::Error)` - `Process`

- [ ] Évaluer si `anyhow` est utile
      `thiserror` est adapté pour les erreurs structurées du projet.
      `anyhow` peut être utile pour ajouter rapidement du contexte dans un binaire CLI,
      mais on peut commencer avec `thiserror` seul.

- [ ] Faire cette refacto au bon moment
      Pas maintenant au milieu de l’archive. Bon moment possible : - après l’archive complète - avant compression - ou avant FTP/SFTP, quand les erreurs réseau/process deviendront plus nombreuses.

## Renommer le nom du CLI

- [x] Renommer le projet et le binaire en `pack`

## Changelog / release

- [x] Mettre en place Cocogitto pour valider les Conventional Commits.
      Fait : ajout de `cog.toml`, d'un hook `.githooks/commit-msg` qui lance `cog verify --file`,
      et d'une documentation rapide dans `CONTRIBUTING.md`.

- [x] Vérifier les anciens commits avec Cocogitto.
      Fait : `cog check --from-latest-tag` a identifié 11 commits non conformes depuis `v0.1.0`,
      puis une nouvelle vérification après correction a confirmé : `No errored commits`.

- [x] Faire une repasse sur l'historique non conforme.
      Fait : les 11 commits depuis `v0.1.0` ont été réécrits en Conventional Commits via rebase interactif,
      puis poussés sur la remote avec `git push --force-with-lease`. Attention pour la prochaine fois :
      cette opération réécrit l'historique Git et change les SHA des commits concernés et de leurs enfants.
      Référence : <https://docs.cocogitto.io/guide/edit.html>

- [ ] Évaluer si Cocogitto suffit pour le changelog automatique.
      Décision actuelle : commencer avec Cocogitto plutôt que git-cliff, car il couvre déjà la validation
      Conventional Commits, le bump de version et la génération de changelog. Garder git-cliff comme option
      future seulement si le changelog de Cocogitto devient trop limité.
