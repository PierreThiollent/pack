# TODO — Améliorations repérées en cours de route

## Temp directory

- [ ] Utiliser un dossier unique par run (comme GoBackup avec `os.MkdirTemp`)
      Au lieu de `/tmp/rucksack/{model}/` fixe, créer `/tmp/rucksackXXXXX/{timestamp}/{model}/`
      Évite les collisions si deux `perform` sont lancés en même temps
