# TODO — Améliorations repérées en cours de route

## Temp directory

- [ ] Utiliser un dossier unique par run
      Au lieu de `/tmp/rbak/{model}/` fixe, créer `/tmp/rbakXXXXX/{timestamp}/{model}/`
      Évite les collisions si deux `perform` sont lancés en même temps

- [x] Ajouter un `workdir` configurable
      Comme GoBackup, permettre de choisir la racine des fichiers temporaires :
      `workdir: /var/tmp/rbak`.
      Le dossier unique par run serait ensuite créé à l’intérieur de ce `workdir`.

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

## Renommer le nom du CLI

- [x] Renommer le projet et le binaire en `rbak`
