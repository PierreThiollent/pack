# Agent conventions for rbak

This file defines conventions that the coding agent (Pi) must follow when working on this project.

## Language

- **Code** : all Rust code written in English (identifiers, comments, docstrings)
- **Test names** : English (`parse_config_with_one_model`, not `parse_config_avec_un_model`)
- **Git commits** : English messages
- **User-facing documentation** (README, comments explaining concepts to Pierre) : **French**, since Pierre is a French native speaker
- **Plan.md, session logs, discussions** : French

## Commits

- Do not commit unless Pierre explicitly asks to commit
- Use meaningful English commit messages

## Code style

- Run `cargo fmt` before each commit (enforced by pre-commit hook)
- Follow idiomatic Rust conventions
- Add tests alongside new code
- **No `let _ =` on `Result`** : always handle or propagate errors, never silently discard them
- **No abbreviations in variable names** : `queue` not `q`, `message` not `msg`, `channel` not `ch`. Common abbreviations like `config` are fine
- **No `mod.rs` files** : use `src/module.rs` as the module root instead of `src/module/mod.rs`
