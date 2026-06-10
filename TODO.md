# TODO — Améliorations repérées en cours de route

## Temp directory

- [ ] Utiliser un dossier unique par run
      Au lieu de `/tmp/rbak/{model}/` fixe, créer `/tmp/rbakXXXXX/{timestamp}/{model}/`
      Évite les collisions si deux `perform` sont lancés en même temps

## Renommer le nom du CLI

- [x] Renommer le projet et le binaire en `rbak`
